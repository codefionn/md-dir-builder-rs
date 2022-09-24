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
use std::{sync::Arc, collections::HashMap};

use ahash::RandomState;
use futures::{StreamExt, stream::SplitSink, SinkExt};
use tokio::{sync::{self, Mutex}, task};
use chrono::prelude::*;

use axum::{
    body::{Bytes, Full},
    http::{StatusCode, Uri},
    response::{IntoResponse, Html, Response},
    routing::{get, post},
    Json, Router, extract::{WebSocketUpgrade, ws::{WebSocket, Message}}, Extension
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
        async fn $name() -> Response<Full<Bytes>> {
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", $type)
                .body(Full::from(bytes::Bytes::from_static(include_bytes!($path))))
                .unwrap()
        }
    };
}

get_resource_generator!(ws_js_file, "application/javascript", "./ui/ws.js");

#[derive(Clone)]
struct WsState {
    pub ws_channels: Arc<Mutex<HashMap<i64, sync::mpsc::Sender<MsgSrv>, RandomState>>> 
}

pub async fn create_router(
    tx_file: sync::mpsc::Sender<MsgBuilder>,
) -> (Router, tokio::sync::mpsc::Sender<MsgSrv>) {
    let (tx, mut rx) = sync::mpsc::channel(128);
    let ws_channels: Arc<Mutex<HashMap<i64, sync::mpsc::Sender<MsgSrv>, RandomState>>> 
        = Arc::new(Mutex::new(HashMap::with_capacity_and_hasher(4, RandomState::new())));

    let ws_channels_for_listener = ws_channels.clone();
    task::spawn(async move {
        while let Some(msg) = rx.recv().await {
            log::debug!("Server event: {:?}", msg);

            match msg {
                MsgSrv::File(path, content) => {
                    let ws_channels = ws_channels_for_listener.lock().await;
                    log::debug!("Open websockets: {}", ws_channels.len());
                    for tx_ws in ws_channels.values() {
                        tx_ws.send(MsgSrv::File(path.clone(), content.clone())).await.unwrap();
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
        .route("/.ws", get(handle_ws))
        .layer(Extension (WsState { ws_channels: ws_channels.clone() }))
        .route("/.ping", get(ping))
        .route("/.api", post(|| async {}))
        .fallback(get(|uri: Uri| async move {
            let (tx_onefile, rx_onefile) = sync::oneshot::channel();
            tx_file
                .clone()
                .send(MsgBuilder::File(uri.path().to_string(), tx_onefile))
                .await
                .unwrap_or_else(|_| panic!("Failed awaiting result"));
    
            if let Ok(result) = rx_onefile.await {
                match result {
                    (Some(result), files) => {
                        let result = format!("{}", crate::ui::render_page(uri.path(), crate::ui::Contents::Html(result.as_str()), &files[..]).into_string());
    
                        (StatusCode::OK, Html(result))
                    },
                    (None, files) => {
                        let result = format!("{}", crate::ui::render_page(uri.path(), crate::ui::Contents::NotFound(), &files[..]).into_string());
    
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

async fn handle_ws_socket(mut socket: WebSocket, state: WsState) {
    log::debug!("Established websocket connection");

    let (tx_ws, mut rx_ws) = sync::mpsc::channel(128);
    let (mut sender, mut receiver) = socket.split();

    task::spawn(async move {
        while let Some(msg) = rx_ws.recv().await {
            log::debug!("WebSocket Channel received: {:?}", msg);
        
            match msg {
                MsgSrv::File(path, content) => {
                    // Send the client update of the content
                    if let Err(err) = send_msg(&mut sender, json::object! {
                        action: "update-content",
                        path: path,
                        content: content
                    }).await {
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
        let msg = if let Ok(msg) = msg {
            msg
        } else {
            // client disconnected
            tx_ws.send(MsgSrv::Exit()).await.unwrap();
            break;
        };
    }

    state.ws_channels.lock().await.remove_entry(&id);

    log::debug!("Closed websocket connection");
}

async fn send_msg(sender: &mut SplitSink<WebSocket, Message>, val: json::JsonValue) -> anyhow::Result<()> {
    sender.send(Message::Text(val.dump())).await?;

    Ok(())
}
