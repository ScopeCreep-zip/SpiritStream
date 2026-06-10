//! Connector integration harness — mock-server round-trips (Q1/Q2).
//!
//! Each submodule stands up a local mock for one connector and drives a
//! full lifecycle against it: connect → receive a message → (send, where
//! the platform supports it) → disconnect. Every connector is built with
//! [`ChatEndpoints::for_mock`](super::ChatEndpoints), so the production
//! endpoint constants are never touched — the same dependency-injection
//! seam the factory uses in production redirects all traffic to
//! `127.0.0.1` here.
//!
//! Two connectors are deliberately *not* covered by a full round-trip,
//! because neither exposes a server-address override a mock could bind to:
//! - **Twitch IRC ride** — the chat socket lives inside the `twitch-irc`
//!   crate, which dials `irc.chat.twitch.tv` with no host override. Only
//!   Twitch's HTTP seams (GQL channel lookup + OAuth validate) are
//!   injectable, so [`twitch`] exercises the GQL seam (channel-not-found
//!   rejection) and stops before the IRC client is constructed.
//! - **TikTok** — the entire connection lives inside `piratetok-live-rs`,
//!   whose builder takes only a username. There is no endpoint to rebase,
//!   so TikTok has no mock round-trip; its read-only contract is pinned in
//!   `connector_tests.rs` instead.
//!
//! See `docs/04-streaming/06-chat-platforms.md` for the per-platform
//! send/receive + injectability matrix.

mod facebook;
mod kick;
mod trovo;
mod twitch;
mod youtube;

use std::future::Future;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_async, WebSocketStream};

use crate::models::ChatMessage;

/// Server side of a mock WebSocket connection (no TLS — the harness binds
/// plain `ws://127.0.0.1`).
pub(super) type MockWs = WebSocketStream<TcpStream>;

/// Bind a single-connection mock WebSocket server on an ephemeral local
/// port and run `handler` against the one client that connects. Returns
/// the `ws://` URL to hand to [`ChatEndpoints::for_mock`](super::ChatEndpoints)
/// and a handle whose `bool` output is the handler's verdict — by
/// convention `true` means the client closed the socket cleanly (no
/// zombie read loop left running after `disconnect()`).
pub(super) async fn spawn_mock_ws<F, Fut>(handler: F) -> (String, JoinHandle<bool>)
where
    F: FnOnce(MockWs) -> Fut + Send + 'static,
    Fut: Future<Output = bool> + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock websocket listener");
    let addr = listener.local_addr().expect("mock websocket local addr");
    let url = format!("ws://{addr}");
    let handle = tokio::spawn(async move {
        let (stream, _) = match listener.accept().await {
            Ok(pair) => pair,
            Err(_) => return false,
        };
        let ws = match accept_async(stream).await {
            Ok(ws) => ws,
            Err(_) => return false,
        };
        handler(ws).await
    });
    (url, handle)
}

/// Read the next text frame from the mock socket, skipping ping/pong and
/// binary frames. Returns `None` if the client closes or the stream
/// errors before a text frame arrives.
pub(super) async fn next_text(ws: &mut MockWs) -> Option<String> {
    while let Some(frame) = ws.next().await {
        match frame {
            Ok(Message::Text(text)) => return Some(text),
            Ok(Message::Close(_)) | Err(_) => return None,
            Ok(_) => continue,
        }
    }
    None
}

/// Consume frames until the client sends a Close (clean disconnect) or the
/// stream ends. `true` == an explicit Close frame was observed, which is
/// the harness's proxy for "the connector's read loop shut down rather
/// than leaking".
pub(super) async fn await_client_close(mut ws: MockWs) -> bool {
    while let Some(frame) = ws.next().await {
        match frame {
            Ok(Message::Close(_)) => return true,
            Ok(_) => continue,
            Err(_) => return false,
        }
    }
    false
}

/// Receive one chat message with a generous timeout so a wedged connector
/// fails the test instead of hanging the suite.
pub(super) async fn recv_one(rx: &mut mpsc::Receiver<ChatMessage>) -> Option<ChatMessage> {
    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .ok()
        .flatten()
}

/// Assert the mock WS server task finished and saw a clean client Close.
pub(super) async fn assert_ws_closed(handle: JoinHandle<bool>) {
    let closed = tokio::time::timeout(Duration::from_secs(5), handle)
        .await
        .expect("mock websocket server task did not finish — connector leaked the socket")
        .expect("mock websocket server task panicked");
    assert!(
        closed,
        "connector did not send a Close frame on disconnect (zombie read loop)"
    );
}
