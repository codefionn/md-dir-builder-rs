/*
 *  md-dir-builder serve markdown files in a given directory
 *  Copyright (C) 2022 Fionn Langhans
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU General Public License for more details.
 *
 *  You should have received a copy of the GNU General Public License
 */
use std::{collections::HashMap, sync::Arc};

use crc::Crc;

use ahash::RandomState;
use futures::{stream::SplitSink, SinkExt, StreamExt};
use tokio::{
    sync::{self, Mutex},
    task,
};

use axum::headers::{ETag, IfNoneMatch};
use axum::{
    body::{Bytes, Full},
    extract::{
        ws::{Message, WebSocket},
        TypedHeader, WebSocketUpgrade,
    },
    http::{StatusCode, Uri},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};

use serde::{Deserialize, Serialize};

use super::{MsgBuilder, MsgSrv};

#[derive(Serialize, Deserialize)]
struct PingResponse {
    pub success: bool,
    pub msg: String,
}

async fn ping() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(PingResponse {
            success: true,
            msg: format!("Pong"),
        }),
    )
}

macro_rules! get_resource_generator {
    ($name:ident, $type:literal, $path:literal) => {
        async fn $name(if_none_match: Option<TypedHeader<IfNoneMatch>>) -> Response<Full<Bytes>> {
            let bytes = include_bytes!($path);
            let hasher = Crc::<u64>::new(&crc::CRC_64_XZ);
            let etag_hash = format!("\"{}\"", hasher.checksum(bytes).to_string());
            let etag: ETag = etag_hash.parse().unwrap();

            if let Some(TypedHeader(if_none_match)) = if_none_match {
                log::debug!("Server({:?}) =? Client({:?})", etag, if_none_match);
                if if_none_match == IfNoneMatch::from(etag) {
                    log::debug!("ETag check passed");
                    return Response::builder()
                        .status(StatusCode::NOT_MODIFIED)
                        .body(Full::from(bytes::Bytes::from_static(b"")))
                        .unwrap();
                }
            }

            Response::builder()
                .status(StatusCode::OK)
                .header("ETag", etag_hash)
                .header("Content-Type", $type)
                .body(Full::from(bytes::Bytes::from_static(bytes)))
                .unwrap()
        }
    };
}

macro_rules! get_full_text_page {
    ($path:literal, $tx_file:ident) => {{
        let tx_file = $tx_file.clone();

        get(|| async move {
            let (tx_onefile, rx_onefile) = sync::oneshot::channel();

            tx_file
                .clone()
                .send(MsgBuilder::AllFiles(tx_onefile))
                .await
                .unwrap_or_else(|_| panic!("Failed awaiting result"));

            if let Ok(all_files) = rx_onefile.await {
                let result = crate::ui::render_page(
                    "License",
                    crate::ui::Contents::Text(include_str!($path)),
                    &all_files[..],
                );
                (StatusCode::OK, Html(format!("{}", result.into_string())))
            } else {
                (StatusCode::GONE, Html(format!("Internal server error")))
            }
        })
    }};
}

get_resource_generator!(ws_js_file, "application/javascript", "./ui/ws.js");
get_resource_generator!(prism_js_file, "application/javascript", "./ui/prism.js");

#[derive(Clone)]
struct WsState {
    pub ws_channels: Arc<Mutex<HashMap<i64, sync::mpsc::Sender<MsgSrv>, RandomState>>>,
}

fn determine_real_path(path: &str) -> String {
    return format!(
        "/{}",
        path.split('/')
            .map(|part| urlencoding::decode(part).unwrap().to_string())
            .fold(String::new(), |a, b| {
                if a.is_empty() {
                    b
                } else {
                    format!("{}/{}", a, b)
                }
            })
    );
}

pub async fn create_router(
    tx_file: sync::mpsc::Sender<MsgBuilder>,
) -> (Router, tokio::sync::mpsc::Sender<MsgSrv>) {
    let (tx, mut rx) = sync::mpsc::channel(128);
    let ws_channels: Arc<Mutex<HashMap<i64, sync::mpsc::Sender<MsgSrv>, RandomState>>> = Arc::new(
        Mutex::new(HashMap::with_capacity_and_hasher(4, RandomState::new())),
    );

    let ws_channels_for_listener = ws_channels.clone();
    task::spawn(async move {
        while let Some(msg) = rx.recv().await {
            log::debug!("Server event: {:?}", msg);

            match msg {
                MsgSrv::File(path, content) => {
                    let ws_channels = ws_channels_for_listener.lock().await;
                    log::debug!("Open websockets: {}", ws_channels.len());
                    for tx_ws in ws_channels.values() {
                        tx_ws
                            .send(MsgSrv::File(path.clone(), content.clone()))
                            .await
                            .unwrap();
                    }
                }
                MsgSrv::NewFile(path, all_files) => {
                    let ws_channels = ws_channels_for_listener.lock().await;
                    log::debug!("Open websockets: {}", ws_channels.len());
                    for tx_ws in ws_channels.values() {
                        tx_ws
                            .send(MsgSrv::NewFile(path.clone(), all_files.clone()))
                            .await
                            .unwrap();
                    }
                }
                MsgSrv::Exit() => {
                    break;
                }
            }
        }
    });

    let router = Router::new()
        //.route("/.rsc/Roboto/Roboto-Regular.ttf", get(rsc_roboto_regular))
        .route("/.rsc/ws.js", get(ws_js_file))
        .route("/.rsc/prism.js", get(prism_js_file))
        .route("/.ws", get(handle_ws))
        .layer(Extension(WsState {
            ws_channels: ws_channels.clone(),
        }))
        .route("/.ping", get(ping))
        .route("/.api", post(|| async {}))
        .route("/.license", get_full_text_page!("../LICENSE", tx_file))
        .route("/.contents/*rest", {
            let tx_file = tx_file.clone();
            get(|uri: Uri| async move {
                let requested_file = uri.path()["/.contents".len()..].to_string();
                let requested_file = determine_real_path(&requested_file);

                log::debug!("Requested file contents: {}", requested_file);
                let (tx_onefile, rx_onefile) = sync::oneshot::channel();
                tx_file
                    .clone()
                    .send(MsgBuilder::File(requested_file.clone(), tx_onefile))
                    .await
                    .unwrap_or_else(|_| panic!("Failed awaiting result"));

                if let Ok(result) = rx_onefile.await {
                    match result {
                        (Some(result), _files) => {
                            let result = format!(
                                "{}",
                                crate::ui::render_contents(crate::ui::Contents::Html(
                                    result.as_str()
                                ))
                                .into_string()
                            );

                            (StatusCode::OK, Html(result))
                        }
                        (None, _files) => {
                            let result = format!(
                                "{}",
                                crate::ui::render_contents(crate::ui::Contents::NotFound())
                                    .into_string()
                            );

                            (StatusCode::NOT_FOUND, Html(result))
                        }
                    }
                } else {
                    (StatusCode::GONE, Html(format!("Internal server error")))
                }
            })
        })
        .fallback(get(|uri: Uri| async move {
            let requested_file = uri.path().to_string();
            let requested_file = determine_real_path(&requested_file);

            log::debug!("Requested file: {}", requested_file);
            let (tx_onefile, rx_onefile) = sync::oneshot::channel();
            tx_file
                .clone()
                .send(MsgBuilder::File(requested_file.clone(), tx_onefile))
                .await
                .unwrap_or_else(|_| panic!("Failed awaiting result"));

            if let Ok(result) = rx_onefile.await {
                match result {
                    (Some(result), files) => {
                        let result = format!(
                            "{}",
                            crate::ui::render_page(
                                requested_file.as_str(),
                                crate::ui::Contents::Html(result.as_str()),
                                &files[..]
                            )
                            .into_string()
                        );

                        (StatusCode::OK, Html(result))
                    }
                    (None, files) => {
                        let result = format!(
                            "{}",
                            crate::ui::render_page(
                                requested_file.as_str(),
                                crate::ui::Contents::NotFound(),
                                &files[..]
                            )
                            .into_string()
                        );

                        (StatusCode::NOT_FOUND, Html(result))
                    }
                }
            } else {
                (StatusCode::GONE, Html(format!("Internal server error")))
            }
        }));

    (router, tx)
}

async fn handle_ws(ws: WebSocketUpgrade, Extension(state): Extension<WsState>) -> Response {
    log::debug!("Trying to establish websocket connection");
    ws.on_upgrade(|socket| handle_ws_socket(socket, state))
}

async fn handle_ws_socket(socket: WebSocket, state: WsState) {
    log::debug!("Established websocket connection");

    let (tx_ws, mut rx_ws) = sync::mpsc::channel(128);
    let (mut sender, mut receiver) = socket.split();

    task::spawn(async move {
        while let Some(msg) = rx_ws.recv().await {
            log::debug!("WebSocket Channel received: {:?}", msg);

            match msg {
                MsgSrv::File(path, content) => {
                    // Send the client update of the content
                    if let Err(err) = send_msg(
                        &mut sender,
                        json::object! {
                            action: "update-content",
                            path: path,
                            content: content
                        },
                    )
                    .await
                    {
                        log::error!("Web socket connection broke: {}", err);
                        break;
                    }
                }
                MsgSrv::NewFile(_, all_files) => {
                    let content = crate::ui::render_sidebar(&all_files[..]);
                    // Send the client update of the sidebar
                    if let Err(err) = send_msg(
                        &mut sender,
                        json::object! {
                            action: "update-sidebar",
                            content: content.into_string()
                        },
                    )
                    .await
                    {
                        log::error!("Web socket connection broke: {}", err);
                        break;
                    }
                }
                MsgSrv::Exit() => {
                    log::debug!("Web socket closed by client");
                    break; // Exit websocket session
                }
            }
        }
    });

    // Announce presence of the web socket
    let id = chrono::offset::Utc::now().timestamp_nanos();
    {
        let mut ws_channels = state.ws_channels.lock().await;
        ws_channels.insert(id, tx_ws.clone());
    }

    while let Some(msg) = receiver.next().await {
        if let Ok(_msg) = msg {
            continue;
        } else {
            // client disconnected
            tx_ws.send(MsgSrv::Exit()).await.unwrap();
            break;
        };
    }

    state.ws_channels.lock().await.remove_entry(&id);

    log::debug!("Closed websocket connection");
}

async fn send_msg(
    sender: &mut SplitSink<WebSocket, Message>,
    val: json::JsonValue,
) -> anyhow::Result<()> {
    sender.send(Message::Text(val.dump())).await?;

    Ok(())
}
