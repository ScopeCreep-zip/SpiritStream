//! Kick connector round-trip against a local Pusher-protocol mock.
//!
//! Drives the full lifecycle: REST chatroom-id lookup → Pusher WS
//! handshake (`connection_established` → `subscribe` → `subscription_succeeded`)
//! → one inbound chat frame → authenticated REST send → disconnect, and
//! asserts the connector sent a Close frame (no leaked read loop).

use futures_util::SinkExt;
use serde_json::json;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::super::{ChatEndpoints, ChatPlatform, KickConnector};
use super::{assert_ws_closed, await_client_close, next_text, recv_one, spawn_mock_ws, MockWs};
use crate::models::{ChatConnectionStatus, ChatCredentials, ChatMessage};

/// Server side of the Kick Pusher socket: completes the subscribe
/// handshake, pushes one chat frame, then waits for the connector's Close.
async fn kick_pusher(mut ws: MockWs) -> bool {
    ws.send(Message::Text(
        json!({ "event": "pusher:connection_established", "data": "{}" }).to_string(),
    ))
    .await
    .expect("send connection_established");

    let subscribe = next_text(&mut ws).await.expect("subscribe frame");
    let parsed: serde_json::Value =
        serde_json::from_str(&subscribe).expect("subscribe frame is JSON");
    assert_eq!(parsed["event"].as_str(), Some("pusher:subscribe"));
    assert_eq!(
        parsed["data"]["channel"].as_str(),
        Some("chatrooms.777.v2"),
        "connector must subscribe to the chatroom id from the REST lookup",
    );

    ws.send(Message::Text(
        json!({
            "event": "pusher_internal:subscription_succeeded",
            "channel": "chatrooms.777.v2",
            "data": "{}",
        })
        .to_string(),
    ))
    .await
    .expect("send subscription_succeeded");

    let chat_inner = json!({
        "id": "k1",
        "content": "hello kick",
        "sender": { "username": "KickFan" },
    })
    .to_string();
    ws.send(Message::Text(
        json!({ "event": "App\\Events\\ChatMessageEvent", "data": chat_inner }).to_string(),
    ))
    .await
    .expect("send chat frame");

    await_client_close(ws).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn kick_round_trip_connect_receive_send_disconnect() {
    let http = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/channels/teststreamer"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "chatroom": { "id": 777 } })),
        )
        .mount(&http)
        .await;
    Mock::given(method("POST"))
        .and(path("/public/v1/chat"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "data": { "is_sent": true } })),
        )
        .mount(&http)
        .await;

    let (ws_url, ws_handle) = spawn_mock_ws(kick_pusher).await;
    let endpoints = ChatEndpoints::for_mock(&http.uri(), &ws_url);
    let mut kick = KickConnector::with_endpoints(&endpoints);

    let (tx, mut rx) = mpsc::channel::<ChatMessage>(16);
    kick.connect(
        ChatCredentials::Kick {
            channel: "TestStreamer".into(),
            oauth_token: Some("kick-oauth".into()),
            broadcaster_user_id: Some(777),
        },
        tx,
    )
    .await
    .expect("kick connect should succeed against the mock");

    assert_eq!(kick.status(), ChatConnectionStatus::Connected);
    assert!(kick.is_connected());
    assert!(kick.can_send(), "oauth token + broadcaster id enable send");

    let msg = recv_one(&mut rx).await.expect("a kick chat message");
    assert_eq!(msg.message, "hello kick");
    assert_eq!(msg.username, "KickFan");

    kick.send_message("hi from harness".into())
        .await
        .expect("kick send should hit the mock REST endpoint");

    kick.disconnect().await.expect("kick disconnect");
    assert_eq!(kick.status(), ChatConnectionStatus::Disconnected);

    assert_ws_closed(ws_handle).await;
}
