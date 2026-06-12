//! OAuth config truthfulness + the unconfigured-provider pre-flight
//! guard. The dead-link regression: placeholder client IDs used to ride
//! into the authorize URL while `GET /oauth/config` swore everything
//! was configured.
//!
//! The booted test server inherits no SPIRITSTREAM_*_CLIENT_* env (the
//! boot harness controls the child env only for the variables it sets;
//! these tests therefore skip when a developer shell exports real
//! credentials, mirroring the core config tests' guards).

use super::common::{boot, get, post_invoke};
use serde_json::Value;

fn dev_env_clean() -> bool {
    std::env::var("SPIRITSTREAM_TWITCH_CLIENT_ID").is_err()
        && std::env::var("SPIRITSTREAM_KICK_CLIENT_ID").is_err()
}

#[test]
fn oauth_config_reports_placeholder_providers_unconfigured() {
    if !dev_env_clean() {
        return;
    }
    let server = boot();
    let (status, body) = get(&server, "/api/v1/oauth/config");
    assert_eq!(status, 200, "config endpoint must answer: {body}");
    let json: Value = serde_json::from_str(&body).unwrap();
    for provider in [
        "twitchConfigured",
        "youtubeConfigured",
        "kickConfigured",
        "facebookConfigured",
        "trovoConfigured",
    ] {
        assert_eq!(
            json[provider], false,
            "{provider} must be false with placeholder credentials: {json}"
        );
    }
}

#[test]
fn oauth_flow_start_refuses_unconfigured_provider_with_typed_409() {
    if !dev_env_clean() {
        return;
    }
    let server = boot();
    let (status, body) = post_invoke(&server, "/api/v1/oauth/twitch/flow", "{}");
    assert_eq!(
        status, 409,
        "unconfigured provider must be a typed conflict, got {status}: {body}"
    );
    let json: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["kind"], "oauth_provider_not_configured", "{json}");
    assert_eq!(json["details"]["provider"], "twitch", "{json}");
}

#[test]
fn facebook_chat_connect_without_confirm_token_is_rejected() {
    // The identity-warning checkbox in FacebookConnectGate is UX; THIS
    // is the enforcement: a Facebook connect must carry a one-shot
    // `enable_facebook_chat` confirm token or the server refuses it.
    let server = boot();
    let (status, body) = post_invoke(
        &server,
        "/api/v1/chat/connections",
        r#"{"platform":"facebook","enabled":true,"credentials":{"type":"facebook","videoId":"123","accessToken":"tok"}}"#,
    );
    assert!(
        status == 403 || status == 401 || status == 400,
        "facebook connect without confirm token must be rejected, got {status}: {body}"
    );
}
