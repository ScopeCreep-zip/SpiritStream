//! Trovo connector round-trip against a local open-chat WS mock.
//!
//! Drives the full lifecycle: REST chat-token lookup → open-chat WS
//! handshake (`AUTH` → echoed-nonce `RESPONSE`) → one inbound `CHAT`
//! frame → disconnect, and asserts the connector sent a Close frame (no
//! leaked read loop). Trovo is receive-only, so there is no send leg.

use futures_util::SinkExt;
use serde_json::json;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::super::{ChatEndpoints, ChatPlatform, TrovoConnector};
use super::{assert_ws_closed, await_client_close, next_text, recv_one, spawn_mock_ws, MockWs};
use crate::models::{ChatConnectionStatus, ChatCredentials, ChatMessage};

/// Server side of the Trovo open-chat socket: reads the `AUTH` frame,
/// echoes its nonce in a `RESPONSE`, pushes one `CHAT` frame, then waits
/// for the connector's Close.
async fn trovo_ws(mut ws: MockWs) -> bool {
    let auth = next_text(&mut ws).await.expect("AUTH frame");
    let parsed: serde_json::Value = serde_json::from_str(&auth).expect("AUTH frame is JSON");
    assert_eq!(parsed["type"].as_str(), Some("AUTH"));
    let nonce = parsed["nonce"]
        .as_str()
        .expect("AUTH frame carries a nonce")
        .to_string();

    ws.send(Message::Text(
        json!({ "type": "RESPONSE", "nonce": nonce }).to_string(),
    ))
    .await
    .expect("send RESPONSE");

    ws.send(Message::Text(
        json!({
            "type": "CHAT",
            "data": {
                "chats": [
                    {
                        "content": "hello trovo",
                        "nick_name": "TrovoFan",
                        "message_id": "t1",
                        "send_time": 1_700_000_000_i64,
                    }
                ]
            }
        })
        .to_string(),
    ))
    .await
    .expect("send CHAT frame");

    await_client_close(ws).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn trovo_round_trip_connect_receive_disconnect() {
    // The connector reads its client id from the environment. Set it
    // before connect; no other test depends on this var being unset.
    std::env::set_var("SPIRITSTREAM_TROVO_CLIENT_ID", "harness-client-id");

    let http = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/openplatform/chat/channel-token/12345"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "token": "trovo-token" })))
        .mount(&http)
        .await;

    let (ws_url, ws_handle) = spawn_mock_ws(trovo_ws).await;
    let endpoints = ChatEndpoints::for_mock(&http.uri(), &ws_url);
    let mut trovo = TrovoConnector::with_endpoints(&endpoints);

    let (tx, mut rx) = mpsc::channel::<ChatMessage>(16);
    trovo
        .connect(
            ChatCredentials::Trovo {
                channel_id: "12345".into(),
                oauth_token: None,
            },
            tx,
        )
        .await
        .expect("trovo connect should succeed against the mock");

    assert_eq!(trovo.status(), ChatConnectionStatus::Connected);
    assert!(trovo.is_connected());
    assert!(!trovo.can_send(), "trovo is receive-only");

    let msg = recv_one(&mut rx).await.expect("a trovo chat message");
    assert_eq!(msg.message, "hello trovo");
    assert_eq!(msg.username, "TrovoFan");

    trovo.disconnect().await.expect("trovo disconnect");
    assert_eq!(trovo.status(), ChatConnectionStatus::Disconnected);

    assert_ws_closed(ws_handle).await;
}
