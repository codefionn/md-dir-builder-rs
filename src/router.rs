use std::sync::Arc;

use tokio::{sync, task};

use axum::{
    body::{Bytes, Full},
    http::{StatusCode, Uri},
    response::{IntoResponse, Html, Response},
    routing::{get, post},
    Json, Router
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

pub async fn create_router(
    tx_file: sync::mpsc::Sender<MsgBuilder>,
) -> (Router, tokio::sync::mpsc::Sender<MsgSrv>) {
    let (tx, mut rx) = tokio::sync::mpsc::channel(128);

    task::spawn(async move {
        while let Some(msg) = rx.recv().await {
            log::debug!("{:?}", msg);

            match msg {
                MsgSrv::File(path, content) => {}
                MsgSrv::Exit() => {}
            }
        }
    });

    (
        Router::new()
            //.route("/.rsc/Roboto/Roboto-Regular.ttf", get(rsc_roboto_regular))
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
            })),
        tx,
    )
}
