//! Twitch Helix chat (room) settings.
//!
//! Applies the profile's follower-only default at chat-connect time via
//! `PATCH /helix/chat/settings`. Requires the
//! `moderator:manage:chat_settings` OAuth scope; the token's owner acts
//! as both `broadcaster_id` and `moderator_id` (the streamer applies the
//! setting to their own channel — exactly the safety-wizard use case).
//!
//! Both the user id / client id AND the scope set come from the
//! `/oauth2/validate` probe, so the helper is self-contained: it needs
//! only the endpoints struct and the bearer token. A missing scope is a
//! structured error (`follower_only_missing_scope`) the caller surfaces
//! as a `follower_only_unsupported` event — fail loud, never pretend
//! the protection is active.

use std::time::Duration;

use crate::errors::{CoreError, ValidationIssue};
use crate::services::chat::ChatEndpoints;

pub(crate) const FOLLOWER_ONLY_SCOPE: &str = "moderator:manage:chat_settings";

#[derive(serde::Deserialize)]
struct ValidateResponse {
    client_id: String,
    user_id: String,
    #[serde(default)]
    scopes: Vec<String>,
}

fn missing_scope_error() -> CoreError {
    CoreError::ValidationFailed {
        reasons: vec![ValidationIssue {
            code: "follower_only_missing_scope".into(),
            message: format!(
                "The Twitch token lacks the {FOLLOWER_ONLY_SCOPE} scope — reconnect \
                 Twitch to grant it."
            ),
            path: None,
        }],
    }
}

/// Turn on follower-only mode for the token owner's own channel.
pub(crate) async fn apply_follower_only_default(
    endpoints: &ChatEndpoints,
    token: &str,
) -> Result<(), CoreError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| CoreError::NetworkError {
            detail: format!("Failed to create HTTP client: {e}"),
        })?;

    // 1. Validate the token: yields user_id (broadcaster == moderator ==
    //    token owner), the client id the Helix call must echo, and the
    //    granted scope set.
    let response = client
        .get(&endpoints.twitch_validate)
        .header("Authorization", format!("OAuth {token}"))
        .send()
        .await
        .map_err(|e| CoreError::NetworkError {
            detail: format!("Twitch token validation failed: {e}"),
        })?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(CoreError::Unauthorized);
    }
    if !response.status().is_success() {
        return Err(CoreError::NetworkError {
            detail: format!("Twitch token validation failed: {}", response.status()),
        });
    }
    let validated: ValidateResponse =
        response.json().await.map_err(|e| CoreError::NetworkError {
            detail: format!("Twitch validate response parse failed: {e}"),
        })?;

    if !validated
        .scopes
        .iter()
        .any(|s| s == FOLLOWER_ONLY_SCOPE)
    {
        return Err(missing_scope_error());
    }

    // 2. PATCH the channel's chat settings. `follower_mode_duration` is
    //    deliberately omitted: Twitch then keeps the channel's existing
    //    duration (or its 0-minute default for first-time enables).
    let response = client
        .patch(&endpoints.twitch_helix_chat_settings)
        .query(&[
            ("broadcaster_id", validated.user_id.as_str()),
            ("moderator_id", validated.user_id.as_str()),
        ])
        .header("Authorization", format!("Bearer {token}"))
        .header("Client-Id", &validated.client_id)
        .json(&serde_json::json!({ "follower_mode": true }))
        .send()
        .await
        .map_err(|e| CoreError::NetworkError {
            detail: format!("Twitch chat-settings update failed: {e}"),
        })?;

    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        // Helix rejects despite the validate probe (revoked mid-flight,
        // scope mismatch on Twitch's side) — same remediation as a
        // missing scope: reconnect Twitch.
        return Err(missing_scope_error());
    }
    if !status.is_success() {
        return Err(CoreError::NetworkError {
            detail: format!("Twitch chat-settings update failed: {status}"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn endpoints_for(server: &MockServer) -> ChatEndpoints {
        ChatEndpoints::for_mock(&server.uri(), "ws://127.0.0.1:1")
    }

    fn validate_body(scopes: &[&str]) -> serde_json::Value {
        serde_json::json!({
            "client_id": "client-abc",
            "login": "streamer",
            "user_id": "12345",
            "scopes": scopes,
            "expires_in": 3600,
        })
    }

    #[tokio::test]
    async fn applies_follower_mode_when_scope_present() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/oauth2/validate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(validate_body(&[
                "chat:read",
                "chat:edit",
                FOLLOWER_ONLY_SCOPE,
            ])))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("PATCH"))
            .and(path("/helix/chat/settings"))
            .and(query_param("broadcaster_id", "12345"))
            .and(query_param("moderator_id", "12345"))
            .and(header("Client-Id", "client-abc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{ "broadcaster_id": "12345", "follower_mode": true }]
            })))
            .expect(1)
            .mount(&server)
            .await;

        apply_follower_only_default(&endpoints_for(&server), "tok")
            .await
            .expect("follower mode should apply");
    }

    #[tokio::test]
    async fn missing_scope_fails_loud_without_touching_helix() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/oauth2/validate"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(validate_body(&["chat:read", "chat:edit"])),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("PATCH"))
            .and(path("/helix/chat/settings"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;

        let err = apply_follower_only_default(&endpoints_for(&server), "tok")
            .await
            .expect_err("missing scope must fail");
        let CoreError::ValidationFailed { reasons } = err else {
            panic!("expected ValidationFailed, got {err:?}");
        };
        assert_eq!(reasons[0].code, "follower_only_missing_scope");
    }

    #[tokio::test]
    async fn rejected_token_maps_to_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/oauth2/validate"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let err = apply_follower_only_default(&endpoints_for(&server), "bad")
            .await
            .expect_err("rejected token must fail");
        assert!(matches!(err, CoreError::Unauthorized));
    }
}
