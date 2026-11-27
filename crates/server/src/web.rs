//! HTTP and WebSocket: the embedded dev page and `/ws`.

use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::header;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use tokio::net::TcpListener;
use tokio::sync::broadcast::{self, error::RecvError};

use crate::connection::{Change, Connection, Server};
use crate::protocol::{ServerMessage, encode};

macro_rules! page_file {
    ($file:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/web/dev/",
            $file
        ))
    };
}

const INDEX_HTML: &str = page_file!("index.html");
const APP_JS: &str = page_file!("app.js");
const STYLE_CSS: &str = page_file!("style.css");

/// The routes: `/` (page), `/app.js`, `/style.css` and `/ws` (WebSocket).
pub fn router(server: Arc<Server>) -> Router {
    Router::new()
        .route("/", get(|| async { Html(INDEX_HTML) }))
        .route(
            "/app.js",
            get(|| async { asset("text/javascript", APP_JS) }),
        )
        .route("/style.css", get(|| async { asset("text/css", STYLE_CSS) }))
        .route("/ws", get(websocket))
        .with_state(server)
}

/// Serves [`router`] on `listener` until the process ends.
///
/// # Errors
///
/// An I/O error of the listener.
pub async fn serve(listener: TcpListener, server: Arc<Server>) -> std::io::Result<()> {
    axum::serve(listener, router(server)).await
}

fn asset(content_type: &'static str, body: &'static str) -> Response {
    (
        [
            (
                header::CONTENT_TYPE,
                format!("{content_type}; charset=utf-8"),
            ),
            (header::CACHE_CONTROL, "no-cache".to_owned()),
        ],
        body,
    )
        .into_response()
}

async fn websocket(upgrade: WebSocketUpgrade, State(server): State<Arc<Server>>) -> Response {
    upgrade.on_upgrade(move |socket| run(socket, server))
}

/// One client: text frames in, [`Connection`] replies and match changes out.
async fn run(mut socket: WebSocket, server: Arc<Server>) {
    let mut connection = Connection::new(server);
    let mut changes: Option<broadcast::Receiver<Change>> = None;
    let mut subscribed: Option<u64> = None;
    loop {
        let replies = tokio::select! {
            incoming = socket.recv() => {
                let Some(Ok(frame)) = incoming else { break };
                let replies = match frame {
                    Message::Text(text) => connection.receive(text.as_str()),
                    Message::Close(_) => break,
                    // Pings are answered by axum; binary frames are not part
                    // of protocol v1.
                    _ => continue,
                };
                if connection.match_id() != subscribed {
                    subscribed = connection.match_id();
                    changes = connection.changes();
                }
                replies
            }
            change = next_change(&mut changes) => match change {
                Ok(change) => connection.on_change(&change),
                Err(RecvError::Lagged(_)) => connection.on_lagged(),
                Err(RecvError::Closed) => {
                    changes = None;
                    continue;
                }
            },
        };
        if send(&mut socket, &replies).await.is_err() {
            break;
        }
    }
}

async fn next_change(
    changes: &mut Option<broadcast::Receiver<Change>>,
) -> Result<Change, RecvError> {
    match changes {
        Some(receiver) => receiver.recv().await,
        None => std::future::pending().await,
    }
}

async fn send(socket: &mut WebSocket, messages: &[ServerMessage]) -> Result<(), axum::Error> {
    for message in messages {
        socket.send(Message::Text(encode(message).into())).await?;
    }
    Ok(())
}
