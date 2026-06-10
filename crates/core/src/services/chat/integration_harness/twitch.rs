//! Twitch connector — GQL channel-lookup seam against a wiremock mock.
//!
//! Twitch's chat ride lives inside the `twitch-irc` crate, which dials
//! `irc.chat.twitch.tv` with no host override, so a full connect →
//! receive → send round-trip cannot be mocked the way the other
//! connectors can. Only Twitch's HTTP seams are injectable: the public
//! GQL channel lookup and the OAuth `validate` call. This test exercises
//! the GQL seam via the channel-not-found rejection — the connector
//! returns `InvalidConfig` and records the error *before* the IRC client
//! is ever constructed, so the test is fully hermetic. The IRC leg is
//! covered by the read-only contract pins in `connector_tests.rs`.

use serde_json::json;
use tokio::sync::mpsc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::super::platform::PlatformError;
use super::super::{ChatEndpoints, ChatPlatform, TwitchConnector};
use crate::models::{ChatConnectionStatus, ChatCredentials, ChatMessage};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn twitch_rejects_nonexistent_channel_before_irc() {
    let http = MockServer::start().await;
    // GQL `user(login:)` lookup returns a null user for a channel that
    // does not exist.
    Mock::given(method("POST"))
        .and(path("/gql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": { "user": null } })))
        .mount(&http)
        .await;

    let endpoints = ChatEndpoints::for_mock(&http.uri(), "ws://127.0.0.1:1");
    let mut twitch = TwitchConnector::with_endpoints(&endpoints);

    let (tx, _rx) = mpsc::channel::<ChatMessage>(16);
    let err = twitch
        .connect(
            ChatCredentials::Twitch {
                channel: "ghost".into(),
                auth: None,
            },
            tx,
        )
        .await
        .expect_err("a non-existent channel must be rejected before the IRC ride");

    assert!(
        matches!(err, PlatformError::InvalidConfig(_)),
        "channel-not-found is a config error, got {err:?}",
    );
    assert_eq!(twitch.status(), ChatConnectionStatus::Error);
    assert!(
        twitch
            .last_error()
            .unwrap_or_default()
            .contains("does not exist"),
        "connector should record the channel-not-found reason",
    );
}
