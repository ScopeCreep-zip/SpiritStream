//! Facebook connector round-trip against a wiremock Graph API mock.
//!
//! Facebook Live comments are polled over REST, so there is no WebSocket
//! leg. The connector probes the comments endpoint at connect time
//! (`order=reverse_chronological`, limit 1) before declaring connected,
//! then long-polls (`order=chronological`) for new comments and POSTs to
//! the same endpoint to send. This drives connect → receive → send →
//! disconnect. A dead `ws://127.0.0.1:1` placeholder satisfies `for_mock`
//! and is never dialled.

use serde_json::json;
use tokio::sync::mpsc;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::super::{ChatEndpoints, ChatPlatform, FacebookConnector};
use super::recv_one;
use crate::models::{ChatConnectionStatus, ChatCredentials, ChatMessage};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn facebook_round_trip_connect_receive_send_disconnect() {
    let http = MockServer::start().await;

    // Connect-time credential probe (reverse_chronological, limit 1). Just
    // needs a 2xx so the connector proceeds past the smoke test.
    Mock::given(method("GET"))
        .and(path("/v18.0/vid123/comments"))
        .and(query_param("order", "reverse_chronological"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": [] })))
        .mount(&http)
        .await;

    // Background poll (chronological). `created_time` uses a colon in the
    // timezone offset (`+00:00`) so `parse_from_rfc3339` accepts it — the
    // colon-less `+0000` form is the Q11 bug, deliberately not exercised
    // here.
    Mock::given(method("GET"))
        .and(path("/v18.0/vid123/comments"))
        .and(query_param("order", "chronological"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [
                {
                    "id": "f1",
                    "from": { "name": "FBFan" },
                    "message": "hello facebook",
                    "created_time": "2026-06-02T10:00:00+00:00"
                }
            ]
        })))
        .mount(&http)
        .await;

    Mock::given(method("POST"))
        .and(path("/v18.0/vid123/comments"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "f1_reply" })))
        .mount(&http)
        .await;

    let endpoints = ChatEndpoints::for_mock(&http.uri(), "ws://127.0.0.1:1");
    let mut facebook = FacebookConnector::with_endpoints(&endpoints);

    let (tx, mut rx) = mpsc::channel::<ChatMessage>(16);
    facebook
        .connect(
            ChatCredentials::Facebook {
                video_id: "vid123".into(),
                access_token: "fb-token".into(),
            },
            tx,
        )
        .await
        .expect("facebook connect should succeed against the mock");

    assert_eq!(facebook.status(), ChatConnectionStatus::Connected);
    assert!(facebook.is_connected());
    assert!(facebook.can_send(), "captured token enables send");

    let msg = recv_one(&mut rx).await.expect("a facebook comment");
    assert_eq!(msg.message, "hello facebook");
    assert_eq!(msg.username, "FBFan");

    facebook
        .send_message("hi from harness".into())
        .await
        .expect("facebook send should hit the mock Graph endpoint");

    facebook.disconnect().await.expect("facebook disconnect");
    assert_eq!(facebook.status(), ChatConnectionStatus::Disconnected);
}
