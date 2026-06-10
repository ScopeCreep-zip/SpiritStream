//! YouTube connector round-trip against a wiremock Data API v3 mock.
//!
//! YouTube has no WebSocket leg — it long-polls the live-chat REST
//! resource — so this drives: live-chat discovery (`liveBroadcasts`) →
//! one poll of `liveChat/messages` → authenticated send → disconnect.
//! No mock WS server is needed; `for_mock` still wants a `ws_base`, so a
//! dead `ws://127.0.0.1:1` placeholder is passed and never dialled.

use serde_json::json;
use tokio::sync::mpsc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::super::{ChatEndpoints, ChatPlatform, YouTubeConnector};
use super::recv_one;
use crate::models::{ChatConnectionStatus, ChatCredentials, ChatMessage, YouTubeAuth};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn youtube_round_trip_connect_receive_send_disconnect() {
    let http = MockServer::start().await;

    // OAuth-mode live-chat discovery: first active broadcast's liveChatId.
    Mock::given(method("GET"))
        .and(path("/liveBroadcasts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [ { "snippet": { "liveChatId": "livechat-1" } } ]
        })))
        .mount(&http)
        .await;

    // One poll's worth of messages. A 60s pollingIntervalMillis keeps the
    // background loop from re-polling during the test window. The author
    // channelId differs from the connector's own channel id ("UCtest"), so
    // the self-echo dedup does not swallow this message.
    Mock::given(method("GET"))
        .and(path("/liveChat/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "pollingIntervalMillis": 60000,
            "nextPageToken": "page-2",
            "items": [
                {
                    "id": "y1",
                    "snippet": {
                        "type": "textMessageEvent",
                        "textMessageDetails": { "messageText": "hello youtube" }
                    },
                    "authorDetails": {
                        "displayName": "YTViewer",
                        "channelId": "UCviewer"
                    }
                }
            ]
        })))
        .mount(&http)
        .await;

    Mock::given(method("POST"))
        .and(path("/liveChat/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "sent-1" })))
        .mount(&http)
        .await;

    let endpoints = ChatEndpoints::for_mock(&http.uri(), "ws://127.0.0.1:1");
    let mut youtube = YouTubeConnector::with_endpoints(&endpoints);

    let (tx, mut rx) = mpsc::channel::<ChatMessage>(16);
    youtube
        .connect(
            ChatCredentials::YouTube {
                channel_id: "UCtest".into(),
                auth: YouTubeAuth::AppOAuth {
                    access_token: "yt-access".into(),
                    refresh_token: None,
                    expires_at: None,
                },
            },
            tx,
        )
        .await
        .expect("youtube connect should succeed against the mock");

    assert_eq!(youtube.status(), ChatConnectionStatus::Connected);
    assert!(youtube.is_connected());
    assert!(youtube.can_send(), "app-oauth enables send");

    let msg = recv_one(&mut rx).await.expect("a youtube chat message");
    assert_eq!(msg.message, "hello youtube");
    assert_eq!(msg.username, "YTViewer");

    youtube
        .send_message("hi from harness".into())
        .await
        .expect("youtube send should hit the mock REST endpoint");

    youtube.disconnect().await.expect("youtube disconnect");
    assert_eq!(youtube.status(), ChatConnectionStatus::Disconnected);
}
