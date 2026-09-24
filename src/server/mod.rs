pub mod broadcaster;

pub use broadcaster::UiBroadcaster;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use std::net::SocketAddr;
use tokio::sync::broadcast::error::RecvError;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing::{info, warn};

#[derive(Clone)]
pub struct AppState {
    pub broadcaster: UiBroadcaster,
}

pub async fn start_web_server(
    host: &str,
    port: u16,
    broadcaster: UiBroadcaster,
) -> anyhow::Result<()> {
    let state = AppState { broadcaster };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .nest_service("/", ServeDir::new("frontend"))
        .layer(cors)
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    info!("🚀 [WEB & WS SERVER] Dashboard listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state.broadcaster))
}

async fn handle_socket(mut socket: WebSocket, broadcaster: UiBroadcaster) {
    let mut rx = broadcaster.subscribe();
    info!("🔌 [WS] New Frontend Dashboard Client Connected!");

    loop {
        match rx.recv().await {
            Ok(event) => {
                if let Ok(json_str) = serde_json::to_string(&event) {
                    if socket.send(Message::Text(json_str.into())).await.is_err() {
                        // Client disconnected
                        break;
                    }
                }
            }
            Err(RecvError::Lagged(skipped)) => {
                // High frequency market feed: skip dropped frames smoothly
                tracing::debug!("[WS] Client lagged, skipped {} messages", skipped);
                continue;
            }
            Err(RecvError::Closed) => {
                break;
            }
        }
    }
    info!("🔌 [WS] Frontend Client Disconnected.");
}
