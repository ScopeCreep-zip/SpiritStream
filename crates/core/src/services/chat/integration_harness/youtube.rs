//! YouTube connector round-trip against a wiremock Data API v3 mock.
//!
//! YouTube has no WebSocket leg — it long-polls the live-chat REST
//! resource — so this drives: live-chat discovery (`liveBroadcasts`) →
//! one poll of `liveChat/messages` → authenticated send → disconnect.
//! No mock WS server is needed; `for_mock` still wants a `ws_base`, so a
//! dead `ws://127.0.0.1:1` placeholder is passed and never dialled.

use serde_json::json;
use tokio::sync::mpsc;
use wiremock::matchers::{method, path, query_param};
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

/// REGRESSION GUARD: an UPCOMING/not-yet-live broadcast has a readable chat,
/// but `liveChatMessages.insert` rejects it (HTTP 400 INVALID_REQUEST_METADATA)
/// until it goes live. So the connector must resolve ACTIVE-ONLY and must NEVER
/// query/attach to `upcoming` — attaching gave a "connected but can't send"
/// trap. Here the active query is empty and an upcoming broadcast exists; the
/// connector must fail loud as Disconnected and never touch the upcoming query.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn youtube_upcoming_only_does_not_connect() {
    let http = MockServer::start().await;

    // Nothing live.
    Mock::given(method("GET"))
        .and(path("/liveBroadcasts"))
        .and(query_param("broadcastStatus", "active"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "items": [] })))
        .mount(&http)
        .await;

    // An upcoming broadcast EXISTS — and must NEVER be queried (`expect(0)`,
    // verified on MockServer drop).
    Mock::given(method("GET"))
        .and(path("/liveBroadcasts"))
        .and(query_param("broadcastStatus", "upcoming"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [ { "snippet": { "liveChatId": "livechat-upcoming" } } ]
        })))
        .expect(0)
        .mount(&http)
        .await;

    let endpoints = ChatEndpoints::for_mock(&http.uri(), "ws://127.0.0.1:1");
    let mut youtube = YouTubeConnector::with_endpoints(&endpoints);

    let (tx, _rx) = mpsc::channel::<ChatMessage>(16);
    let result = youtube
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
        .await;

    assert!(
        result.is_err(),
        "must NOT connect to an upcoming-only broadcast (readable but not postable)"
    );
    assert_eq!(
        youtube.status(),
        ChatConnectionStatus::Disconnected,
        "not-live is a calm Disconnected, not Error"
    );
    assert!(!youtube.is_connected());
}

/// The inverse of the regression: connecting to an ACTIVE (live) broadcast
/// yields a chat we can actually POST to. Proves the chat we attach to is
/// send-capable (active = live = readable AND postable).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn youtube_active_broadcast_is_send_capable() {
    let http = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/liveBroadcasts"))
        .and(query_param("broadcastStatus", "active"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [ { "snippet": { "liveChatId": "lc-active", "channelId": "UCowner" } } ]
        })))
        .mount(&http)
        .await;
    Mock::given(method("GET"))
        .and(path("/liveChat/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "pollingIntervalMillis": 60000, "nextPageToken": "p", "items": []
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

    let (tx, _rx) = mpsc::channel::<ChatMessage>(16);
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
        .expect("connect to the active (live) broadcast");

    assert_eq!(youtube.status(), ChatConnectionStatus::Connected);
    assert!(youtube.can_send(), "OAuth on a live broadcast is send-capable");
    youtube
        .send_message("hi from harness".into())
        .await
        .expect("send to a LIVE broadcast succeeds");

    youtube.disconnect().await.expect("youtube disconnect");
}

/// YouTube's `INVALID_REQUEST_METADATA` on insert (the request body is valid;
/// reads work with the same token) is an account-identity problem. The send
/// path must translate it into an ACTIONABLE message — re-sign-in / pick the
/// streaming channel — not echo Google's raw error blob.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn youtube_send_invalid_request_metadata_is_actionable() {
    let http = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/liveBroadcasts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [ { "snippet": { "liveChatId": "lc", "channelId": "UCme" } } ]
        })))
        .mount(&http)
        .await;
    Mock::given(method("GET"))
        .and(path("/liveChat/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "pollingIntervalMillis": 60000, "nextPageToken": "p", "items": []
        })))
        .mount(&http)
        .await;
    // Sending is rejected with YouTube's identity-level error.
    Mock::given(method("POST"))
        .and(path("/liveChat/messages"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": {
                "code": 400,
                "message": "Request contains an invalid argument.",
                "errors": [ { "reason": "INVALID_REQUEST_METADATA" } ],
                "status": "INVALID_ARGUMENT"
            }
        })))
        .mount(&http)
        .await;

    let endpoints = ChatEndpoints::for_mock(&http.uri(), "ws://127.0.0.1:1");
    let mut youtube = YouTubeConnector::with_endpoints(&endpoints);
    let (tx, _rx) = mpsc::channel::<ChatMessage>(16);
    youtube
        .connect(
            ChatCredentials::YouTube {
                channel_id: "UCme".into(),
                auth: YouTubeAuth::AppOAuth {
                    access_token: "yt-access".into(),
                    refresh_token: None,
                    expires_at: None,
                },
            },
            tx,
        )
        .await
        .expect("connect succeeds");

    let err = youtube
        .send_message("hello".into())
        .await
        .expect_err("send must fail on INVALID_REQUEST_METADATA");
    let msg = err.to_string();
    assert!(
        msg.contains("pick the exact channel") && !msg.contains("INVALID_REQUEST_METADATA"),
        "error must be actionable, not the raw Google blob: {msg}"
    );

    youtube.disconnect().await.expect("disconnect");
}

/// Not being live is a normal waiting state, not a failure: when neither an
/// active nor an upcoming broadcast exists, connect must report DISCONNECTED
/// (not ERROR) so the UI stays calm and the reconnect loop doesn't churn.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn youtube_not_live_reports_disconnected_not_error() {
    let http = MockServer::start().await;

    // No broadcast in either state.
    Mock::given(method("GET"))
        .and(path("/liveBroadcasts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "items": [] })))
        .mount(&http)
        .await;

    let endpoints = ChatEndpoints::for_mock(&http.uri(), "ws://127.0.0.1:1");
    let mut youtube = YouTubeConnector::with_endpoints(&endpoints);

    let (tx, _rx) = mpsc::channel::<ChatMessage>(16);
    let result = youtube
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
        .await;

    assert!(result.is_err(), "no broadcast → connect fails");
    assert_eq!(
        youtube.status(),
        ChatConnectionStatus::Disconnected,
        "not-live must be Disconnected, never Error"
    );
    assert!(!youtube.is_connected());
}
