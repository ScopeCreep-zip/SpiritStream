//! RFC 8628 Device Code Flow — the 2026-mandated sign-in path for
//! desktop-class Twitch apps (public client: no secret, no redirect
//! URI, no loopback server). The app shows a short code; the user
//! enters it at the provider's verification page; we poll the token
//! endpoint until the grant lands.

use std::collections::HashMap;
use std::time::Duration;

use log::info;
use serde::{Deserialize, Serialize};

use super::{network, OAuthCompleteResult, OAuthTokens};
use crate::errors::CoreError;

/// What `start_device_flow` hands back. The wire mirrors in the
/// transports expose everything EXCEPT `device_code`, which is the
/// poll credential and never leaves the server process.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthDeviceFlowStart {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
    #[serde(skip_serializing)]
    pub device_code: String,
}

#[derive(Deserialize)]
struct DeviceAuthResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    #[serde(default = "default_interval")]
    interval: u64,
}

fn default_interval() -> u64 {
    5
}

#[derive(Deserialize)]
struct DevicePollError {
    #[serde(default)]
    message: String,
    #[serde(default)]
    error: String,
}

impl super::OAuthService {
    /// Begin a device authorization. Fails loud when the provider is
    /// unconfigured or has no device endpoint (only Twitch does today).
    pub async fn start_device_flow(
        &self,
        provider_name: &str,
    ) -> Result<OAuthDeviceFlowStart, CoreError> {
        if !self.is_configured(provider_name).await {
            return Err(CoreError::OAuthProviderNotConfigured {
                provider: provider_name.to_string(),
            });
        }
        let provider = self.provider_for(provider_name)?;
        let Some(device_url) = provider.device_url.clone() else {
            return Err(CoreError::NotImplemented {
                feature: format!("{provider_name} does not support the device code flow"),
            });
        };
        let client_id = self.client_id_for(provider_name).await;
        let scopes = provider.scopes.join(" ");

        let mut params = HashMap::new();
        params.insert("client_id", client_id.as_str());
        params.insert("scopes", scopes.as_str());

        let response = self
            .http_client
            .post(&device_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| network(format!("device authorization request failed: {e}")))?;
        if !response.status().is_success() {
            let status = response.status();
            // Body dropped without logging — same redaction rationale
            // as `exchange_code` (providers echo submitted fields).
            let _ = response.text().await;
            return Err(network(format!("device authorization failed: {status}")));
        }
        let body: DeviceAuthResponse = response
            .json()
            .await
            .map_err(|e| network(format!("device authorization parse failed: {e}")))?;

        info!(
            "Device flow started for {provider_name}: code expires in {}s, poll every {}s",
            body.expires_in, body.interval
        );
        Ok(OAuthDeviceFlowStart {
            user_code: body.user_code,
            verification_uri: body.verification_uri,
            expires_in: body.expires_in,
            interval: body.interval,
            device_code: body.device_code,
        })
    }

    /// Poll the token endpoint until the user approves (or the code
    /// expires / is denied). Honors the server-given `interval` and the
    /// RFC 8628 `slow_down` backoff. On grant, reuses the same
    /// user-info fetch + result shape as the loopback `complete_flow`,
    /// so persistence is one code path for both flows.
    pub async fn poll_device_flow(
        &self,
        provider_name: &str,
        start: &OAuthDeviceFlowStart,
    ) -> Result<OAuthCompleteResult, CoreError> {
        let provider = self.provider_for(provider_name)?;
        let client_id = self.client_id_for(provider_name).await;
        let mut interval = start.interval.max(1);
        let deadline = std::time::Instant::now() + Duration::from_secs(start.expires_in);

        loop {
            tokio::time::sleep(Duration::from_secs(interval)).await;
            if std::time::Instant::now() >= deadline {
                return Err(CoreError::Unauthorized);
            }

            let mut params = HashMap::new();
            params.insert("client_id", client_id.as_str());
            params.insert("device_code", start.device_code.as_str());
            params.insert("grant_type", "urn:ietf:params:oauth:grant-type:device_code");

            let response = self
                .http_client
                .post(&provider.token_url)
                .form(&params)
                .send()
                .await
                .map_err(|e| network(format!("device token poll failed: {e}")))?;

            if response.status().is_success() {
                let tokens: OAuthTokens = response
                    .json()
                    .await
                    .map_err(|e| network(format!("device token parse failed: {e}")))?;
                let user_info = self.fetch_user_info(provider_name, &tokens.access_token).await?;
                return Ok(OAuthCompleteResult { tokens, user_info });
            }

            // Pending / slow_down arrive as 4xx with a JSON body. Twitch
            // uses `message`; RFC-style providers use `error`.
            let status = response.status();
            let body: DevicePollError = response.json().await.unwrap_or(DevicePollError {
                message: String::new(),
                error: String::new(),
            });
            let signal = if body.error.is_empty() {
                body.message.to_lowercase().replace(' ', "_")
            } else {
                body.error.to_lowercase()
            };
            match signal.as_str() {
                "authorization_pending" => continue,
                "slow_down" => {
                    interval += 5;
                    continue;
                }
                "expired_token" | "expired" => return Err(CoreError::Unauthorized),
                "access_denied" | "authorization_declined" => {
                    return Err(CoreError::Unauthorized)
                }
                other => {
                    return Err(network(format!(
                        "device token poll rejected ({status}): {other}"
                    )))
                }
            }
        }
    }

    /// Resolved client id for a provider — small shared helper for the
    /// device flow (the loopback flow resolves ids inside its own
    /// provider match).
    pub(in crate::services::oauth) async fn client_id_for(&self, provider_name: &str) -> String {
        let config = self.config.lock().await;
        match provider_name {
            "twitch" => config.get_twitch_client_id(),
            "youtube" => config.get_youtube_client_id(),
            "kick" => config.get_kick_client_id(),
            "facebook" => config.get_facebook_client_id(),
            "trovo" => config.get_trovo_client_id(),
            _ => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{OAuthConfig, OAuthProvider, OAuthService};
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn twitch_svc(mock_base: &str) -> OAuthService {
        let svc = OAuthService::new(OAuthConfig {
            twitch_client_id: Some("test-twitch-id".into()),
            ..OAuthConfig::default()
        });
        svc.override_provider(OAuthProvider {
            name: "twitch".into(),
            auth_url: format!("{mock_base}/oauth2/authorize"),
            token_url: format!("{mock_base}/oauth2/token"),
            device_url: Some(format!("{mock_base}/oauth2/device")),
            user_info_url: format!("{mock_base}/helix/users"),
            scopes: vec!["chat:read", "chat:edit"],
        });
        svc
    }

    fn device_start_body() -> serde_json::Value {
        serde_json::json!({
            "device_code": "dev-code-1",
            "user_code": "ABCD-1234",
            "verification_uri": "https://www.twitch.tv/activate",
            "expires_in": 600,
            "interval": 1
        })
    }

    #[tokio::test]
    async fn start_device_flow_returns_user_code_and_keeps_device_code_private() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth2/device"))
            .and(body_string_contains("client_id=test-twitch-id"))
            .respond_with(ResponseTemplate::new(200).set_body_json(device_start_body()))
            .mount(&server)
            .await;

        let svc = twitch_svc(&server.uri());
        let start = svc.start_device_flow("twitch").await.expect("starts");
        assert_eq!(start.user_code, "ABCD-1234");
        assert_eq!(start.verification_uri, "https://www.twitch.tv/activate");
        // The serialized form (what transports mirror) omits device_code.
        let wire = serde_json::to_value(&start).unwrap();
        assert!(wire.get("deviceCode").is_none(), "{wire}");
        assert_eq!(wire["userCode"], "ABCD-1234");
    }

    #[tokio::test]
    async fn start_device_flow_refuses_unconfigured_provider() {
        let svc = OAuthService::new(OAuthConfig::default());
        if std::env::var("SPIRITSTREAM_TWITCH_CLIENT_ID").is_ok() {
            return;
        }
        match svc.start_device_flow("twitch").await {
            Err(crate::errors::CoreError::OAuthProviderNotConfigured { .. }) => {}
            other => panic!("expected OAuthProviderNotConfigured, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn start_device_flow_refuses_provider_without_device_endpoint() {
        let svc = OAuthService::new(OAuthConfig {
            kick_client_id: Some("id".into()),
            kick_client_secret: Some("secret".into()),
            ..OAuthConfig::default()
        });
        match svc.start_device_flow("kick").await {
            Err(crate::errors::CoreError::NotImplemented { .. }) => {}
            other => panic!("expected NotImplemented, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn poll_device_flow_waits_through_pending_then_succeeds() {
        let server = MockServer::start().await;
        // First poll: authorization_pending (Twitch-style `message`).
        Mock::given(method("POST"))
            .and(path("/oauth2/token"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(serde_json::json!({ "message": "authorization_pending", "status": 400 })),
            )
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;
        // Subsequent poll: grant.
        Mock::given(method("POST"))
            .and(path("/oauth2/token"))
            .and(body_string_contains("device_code=dev-code-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "granted-token",
                "refresh_token": "granted-refresh",
                "expires_in": 14400,
                "token_type": "bearer"
            })))
            .mount(&server)
            .await;
        // User-info fetch (Helix /users response shape).
        Mock::given(method("GET"))
            .and(path("/helix/users"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{ "id": "1234", "login": "teststreamer", "display_name": "TestStreamer" }]
            })))
            .mount(&server)
            .await;

        let svc = twitch_svc(&server.uri());
        let start = super::OAuthDeviceFlowStart {
            user_code: "ABCD-1234".into(),
            verification_uri: "https://www.twitch.tv/activate".into(),
            expires_in: 30,
            interval: 1,
            device_code: "dev-code-1".into(),
        };
        let done = svc
            .poll_device_flow("twitch", &start)
            .await
            .expect("grant lands after pending");
        assert_eq!(done.tokens.access_token, "granted-token");
        assert_eq!(done.tokens.refresh_token.as_deref(), Some("granted-refresh"));
        assert_eq!(done.user_info.username, "teststreamer");
    }

    #[tokio::test]
    async fn poll_device_flow_expired_token_is_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth2/token"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(serde_json::json!({ "message": "expired_token" })),
            )
            .mount(&server)
            .await;
        let svc = twitch_svc(&server.uri());
        let start = super::OAuthDeviceFlowStart {
            user_code: "X".into(),
            verification_uri: "u".into(),
            expires_in: 30,
            interval: 1,
            device_code: "dead".into(),
        };
        match svc.poll_device_flow("twitch", &start).await {
            Err(crate::errors::CoreError::Unauthorized) => {}
            other => panic!("expected Unauthorized, got {other:?}"),
        }
    }
}
