use crate::errors::CoreError;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::TcpListener;
use std::time::{Duration, Instant};

use super::pkce::generate_pkce_pair;
use super::provider::OAuthProvider;
use super::tokens::{OAuthCompleteResult, OAuthTokens};
use super::{network, unknown_provider};

/// Result of initiating an OAuth flow. ts-rs-exported so the
/// `@spiritstream/types` package is the single source of truth on
/// the wire shape — no parallel hand-written TS file.
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../../packages/types/src/generated/")]
pub struct OAuthFlowResult {
    /// The URL to open in the browser
    pub auth_url: String,
    /// The port the callback server is listening on
    pub callback_port: u16,
    /// The state parameter for CSRF verification
    pub state: String,
}

/// Pending OAuth flow with PKCE state.
///
/// `state` is also the `HashMap` key the flow is filed under, but we
/// keep a copy inside the value so the callback handler can assert
/// equality between the inbound state param and the stored nonce. Any
/// mismatch indicates an in-memory desync (or tampering) and is
/// refused, even though under normal flow the two are tautologically
/// equal — defense in depth.
#[derive(Debug, Clone)]
pub(super) struct PendingOAuthFlow {
    pub(super) provider: String,
    pub(super) state: String,
    pub(super) code_verifier: String,
    pub(super) redirect_uri: String,
    pub(super) created_at: Instant,
}

impl super::OAuthService {
    /// Find an available port for the callback server.
    /// Uses a small set of fixed ports so that redirect URIs can be pre-registered
    /// with OAuth providers (Twitch requires exact match including port).
    fn find_available_port() -> Result<u16, CoreError> {
        const PREFERRED_PORTS: &[u16] = &[8891, 8892, 8893, 8894, 8895];

        for &port in PREFERRED_PORTS {
            if TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok() {
                return Ok(port);
            }
        }

        for port in 49152..49162 {
            if TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok() {
                return Ok(port);
            }
        }
        Err(CoreError::Internal {
            context: "No available port found for OAuth callback".into(),
        })
    }

    async fn cleanup_expired_flows(&self) {
        let mut flows = self.pending_flows.lock().await;
        let now = Instant::now();
        flows.retain(|_, flow| now.duration_since(flow.created_at) < Duration::from_secs(600));
    }

    /// Build the authorization URL (PKCE optional for code flow).
    fn build_auth_url(
        provider: &OAuthProvider,
        client_id: &str,
        redirect_uri: &str,
        state: &str,
        response_type: &str,
        code_challenge: Option<&str>,
    ) -> String {
        let scopes = provider.scopes.join(" ");

        let mut params = vec![
            ("client_id", client_id),
            ("redirect_uri", redirect_uri),
            ("response_type", response_type),
            ("scope", &scopes),
            ("state", state),
        ];

        if let Some(challenge) = code_challenge {
            params.push(("code_challenge", challenge));
            params.push(("code_challenge_method", "S256"));
        }

        if provider.name == "youtube" {
            params.push(("access_type", "offline"));
            params.push(("prompt", "consent"));
        }

        if provider.name == "twitch" {
            params.push(("force_verify", "true"));
        }

        let query = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        format!("{}?{}", provider.auth_url, query)
    }

    /// Start the OAuth flow for a provider.
    /// Twitch uses implicit flow when no client secret is configured.
    /// YouTube always uses authorization code + PKCE.
    pub async fn start_flow(&self, provider_name: &str) -> Result<OAuthFlowResult, CoreError> {
        self.cleanup_expired_flows().await;

        let config = self.config.lock().await;

        // Fail loud BEFORE binding callback ports or building a URL.
        // Pre-fix, an un-injected placeholder rode into the authorize
        // URL (`client_id=TWITCH_CLIENT_ID_PLACEHOLDER`) and the user's
        // browser opened straight onto the provider's 400 page.
        // (`exchange_code` needs no twin guard: it requires a pending
        // flow, and no flow can start past this check.)
        let configured = match provider_name {
            "twitch" => config.has_twitch(),
            "youtube" => config.has_youtube(),
            "kick" => config.has_kick(),
            "facebook" => config.has_facebook(),
            "trovo" => config.has_trovo(),
            _ => return Err(unknown_provider(provider_name)),
        };
        if !configured {
            return Err(CoreError::OAuthProviderNotConfigured {
                provider: provider_name.to_string(),
            });
        }

        let (provider, client_id, client_secret) = match provider_name {
            "twitch" => (
                OAuthProvider::twitch(),
                config.get_twitch_client_id(),
                config.get_twitch_client_secret(),
            ),
            "youtube" => (
                OAuthProvider::youtube(),
                config.get_youtube_client_id(),
                config.get_youtube_client_secret(),
            ),
            "kick" => (
                OAuthProvider::kick(),
                config.get_kick_client_id(),
                config.get_kick_client_secret(),
            ),
            "facebook" => (
                OAuthProvider::facebook(),
                config.get_facebook_client_id(),
                config.get_facebook_client_secret(),
            ),
            "trovo" => (
                OAuthProvider::trovo(),
                config.get_trovo_client_id(),
                config.get_trovo_client_secret(),
            ),
            _ => return Err(unknown_provider(provider_name)),
        };

        drop(config);

        let port = Self::find_available_port()?;
        let redirect_uri = format!("http://localhost:{}/oauth/callback", port);

        let use_pkce = match provider_name {
            // Twitch does not support PKCE at the token exchange — the
            // loopback auth-code flow only works with a confidential
            // secret override (power users). The shipped PUBLIC client
            // signs in via the Device Code Flow; the old implicit-grant
            // fallback is gone (no refresh token, not the 2026 desktop
            // pattern, and Twitch mandates DCF for desktop apps).
            "twitch" => {
                if client_secret.is_none() {
                    return Err(CoreError::OAuthFlowRequiresDevice {
                        provider: provider_name.to_string(),
                    });
                }
                false
            }
            // Kick mandates auth code + PKCE (OAuth 2.1 — no implicit flow).
            "kick" => true,
            // Facebook's web OAuth uses auth code + App Secret (no PKCE).
            // The token exchange requires both client_id + client_secret.
            "facebook" => false,
            // Trovo: plain auth code + secret (JSON exchange; no PKCE in
            // the Open Platform docs).
            "trovo" => false,
            // YouTube uses auth code + PKCE.
            _ => true,
        };

        let (code_verifier, code_challenge) = if use_pkce {
            let (v, c) = generate_pkce_pair();
            (v, Some(c))
        } else {
            (String::new(), None)
        };

        let state = uuid::Uuid::new_v4().to_string();
        let auth_url = Self::build_auth_url(
            &provider,
            &client_id,
            &redirect_uri,
            &state,
            "code",
            code_challenge.as_deref(),
        );

        let id_len = client_id.len();
        let (id_prefix, id_suffix) = if id_len > 12 {
            (&client_id[..6], &client_id[id_len - 6..])
        } else {
            (client_id.as_str(), client_id.as_str())
        };
        info!(
            "OAuth start: provider={}, client_id_hint={}...{}, len={}",
            provider_name, id_prefix, id_suffix, id_len
        );

        let pending_flow = PendingOAuthFlow {
            provider: provider_name.to_string(),
            state: state.clone(),
            code_verifier,
            redirect_uri: redirect_uri.clone(),
            created_at: Instant::now(),
        };

        {
            let mut flows = self.pending_flows.lock().await;
            flows.insert(state.clone(), pending_flow);
        }

        let flow_label = if use_pkce { "PKCE" } else { "auth code" };
        info!(
            "Starting {} OAuth flow with {} on port {}",
            provider_name, flow_label, port
        );

        Ok(OAuthFlowResult {
            auth_url,
            callback_port: port,
            state,
        })
    }

    /// Exchange an authorization code for tokens (PKCE flow).
    pub async fn exchange_code(
        &self,
        provider_name: &str,
        code: &str,
        state: &str,
    ) -> Result<OAuthTokens, CoreError> {
        let pending_flow = {
            let mut flows = self.pending_flows.lock().await;
            flows.remove(state).ok_or(CoreError::Unauthorized)?
        };

        // CSRF defense in depth: the HashMap key MUST equal the stored
        // state nonce. Under normal flow this is tautological — we
        // inserted with `state.clone()` as both key and value. A
        // divergence here indicates in-memory tampering or a logic bug,
        // and we refuse the callback rather than continuing.
        if pending_flow.state != state {
            error!(
                "OAuth state desync for provider={}: refusing callback",
                provider_name
            );
            return Err(CoreError::Unauthorized);
        }

        if pending_flow.provider != provider_name {
            return Err(CoreError::Internal {
                context: "Provider mismatch in OAuth flow".into(),
            });
        }

        // No configured guard here: an exchange requires a pending flow,
        // and no flow can start past `start_flow`'s guard. Endpoints via
        // `provider_for` (test-overridable); credentials from config.
        let provider = self.provider_for(provider_name)?;

        // Trovo deviates from RFC form-encoding — JSON body + client-id
        // header; the request lives in `oauth/trovo.rs`.
        if provider_name == "trovo" {
            let config = self.config.lock().await;
            let (client_id, client_secret) = (
                config.get_trovo_client_id(),
                config.get_trovo_client_secret().unwrap_or_default(),
            );
            drop(config);
            return self
                .exchange_trovo_code(
                    &provider.token_url,
                    &client_id,
                    &client_secret,
                    code,
                    &pending_flow.redirect_uri,
                )
                .await;
        }

        let config = self.config.lock().await;
        let (client_id, client_secret) = match provider_name {
            "twitch" => (
                config.get_twitch_client_id(),
                config.get_twitch_client_secret(),
            ),
            "youtube" => (
                config.get_youtube_client_id(),
                config.get_youtube_client_secret(),
            ),
            "kick" => (config.get_kick_client_id(), config.get_kick_client_secret()),
            "facebook" => (
                config.get_facebook_client_id(),
                config.get_facebook_client_secret(),
            ),
            _ => return Err(unknown_provider(provider_name)),
        };

        drop(config);

        let mut params = HashMap::new();
        params.insert("client_id", client_id.as_str());
        params.insert("code", code);
        if !pending_flow.code_verifier.is_empty() {
            params.insert("code_verifier", pending_flow.code_verifier.as_str());
        }
        params.insert("grant_type", "authorization_code");
        params.insert("redirect_uri", pending_flow.redirect_uri.as_str());

        let client_secret_owned = client_secret.clone();
        if let Some(ref secret) = client_secret_owned {
            params.insert("client_secret", secret.as_str());
        }

        info!(
            "Exchanging {} authorization code for tokens (PKCE{})",
            provider_name,
            if client_secret.is_some() {
                " + secret"
            } else {
                ""
            }
        );

        let response = self
            .http_client
            .post(provider.token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| network(format!("Token exchange request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            // Deliberately NOT logging the response body. OAuth providers
            // routinely echo the submitted form fields (including
            // `refresh_token`, `client_secret`, `code`) inside error JSON
            // — masking is brittle, omission is correct.
            let _ = response.text().await;
            error!("Token exchange failed: {}", status);
            return Err(network(format!(
                "Token exchange failed: {status}. Please try again."
            )));
        }

        let tokens: OAuthTokens = response.json().await.map_err(|e| CoreError::Internal {
            context: format!("Failed to parse token response: {e}"),
        })?;

        info!("Successfully obtained {} tokens via PKCE", provider_name);
        Ok(tokens)
    }

    /// Complete the OAuth flow: exchange code, fetch user info.
    pub async fn complete_flow(
        &self,
        provider_name: &str,
        code: &str,
        state: &str,
    ) -> Result<OAuthCompleteResult, CoreError> {
        let tokens = self.exchange_code(provider_name, code, state).await?;
        let user_info = self
            .fetch_user_info(provider_name, &tokens.access_token)
            .await?;

        info!(
            "OAuth flow complete for {} user: {}",
            provider_name, user_info.display_name
        );

        Ok(OAuthCompleteResult { tokens, user_info })
    }

    /// Bearer-identified user lookup, normalized into [`OAuthUserInfo`].
    /// Shared by the loopback `complete_flow` and the device flow's
    /// grant handler — one persistence shape for both sign-in paths.
    pub(in crate::services::oauth) async fn fetch_user_info(
        &self,
        provider_name: &str,
        access_token: &str,
    ) -> Result<crate::services::oauth::OAuthUserInfo, CoreError> {
        use crate::services::oauth::OAuthUserInfo;
        let user_info = match provider_name {
            "twitch" => {
                let user = self.fetch_twitch_user(access_token).await?;
                OAuthUserInfo {
                    provider: "twitch".to_string(),
                    user_id: user.id,
                    username: user.login,
                    display_name: user.display_name,
                }
            }
            "youtube" => {
                let channel = self.fetch_youtube_channel(access_token).await?;
                OAuthUserInfo {
                    provider: "youtube".to_string(),
                    user_id: channel.id.clone(),
                    username: channel.id,
                    display_name: channel.title,
                }
            }
            "facebook" => {
                let user = self.fetch_facebook_user(access_token).await?;
                OAuthUserInfo {
                    provider: "facebook".to_string(),
                    user_id: user.id,
                    username: user.name.clone(),
                    display_name: user.name,
                }
            }
            "kick" => {
                let user = self.fetch_kick_user(access_token).await?;
                OAuthUserInfo {
                    provider: "kick".to_string(),
                    user_id: user.user_id.to_string(),
                    username: user.name.clone(),
                    display_name: user.name,
                }
            }
            "trovo" => {
                let user = self.fetch_trovo_user(access_token).await?;
                OAuthUserInfo {
                    provider: "trovo".to_string(),
                    user_id: user.user_id,
                    username: user.user_name,
                    display_name: user.nick_name,
                }
            }
            _ => return Err(unknown_provider(provider_name)),
        };
        Ok(user_info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::CoreError;
    use crate::services::oauth::{OAuthConfig, OAuthService};

    #[test]
    fn build_auth_url_includes_core_params_and_encodes() {
        let p = OAuthProvider::twitch();
        let url = OAuthService::build_auth_url(
            &p,
            "cid",
            "http://localhost:8891/oauth/callback",
            "st8",
            "code",
            None,
        );
        assert!(url.starts_with("https://id.twitch.tv/oauth2/authorize?"));
        assert!(url.contains("client_id=cid"));
        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A8891%2Foauth%2Fcallback"));
        assert!(url.contains("state=st8"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("force_verify=true"));
        assert!(!url.contains("code_challenge"));
    }

    #[test]
    fn build_auth_url_adds_pkce_challenge_when_present() {
        let p = OAuthProvider::kick();
        let url = OAuthService::build_auth_url(&p, "cid", "http://x/cb", "s", "code", Some("CHAL"));
        assert!(url.contains("code_challenge=CHAL"));
        assert!(url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn build_auth_url_youtube_adds_offline_consent_and_encodes_scope() {
        let p = OAuthProvider::youtube();
        let url = OAuthService::build_auth_url(&p, "cid", "http://x/cb", "s", "code", Some("C"));
        assert!(url.contains("access_type=offline"));
        assert!(url.contains("prompt=consent"));
        assert!(url.contains("scope=https%3A%2F%2Fwww.googleapis.com"));
    }

    #[tokio::test]
    async fn start_flow_unknown_provider_errors() {
        let svc = OAuthService::new(OAuthConfig::default());
        match svc.start_flow("myspace").await {
            Err(CoreError::NotImplemented { feature }) => assert!(feature.contains("myspace")),
            other => panic!("expected NotImplemented, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn start_flow_kick_uses_pkce_and_registers_pending() {
        let svc = OAuthService::new(OAuthConfig {
            kick_client_id: Some("test-kick-id".into()),
            kick_client_secret: Some("test-kick-secret".into()),
            ..OAuthConfig::default()
        });
        let res = svc.start_flow("kick").await.expect("kick flow starts");
        assert!(res
            .auth_url
            .starts_with("https://id.kick.com/oauth/authorize?"));
        assert!(res.auth_url.contains("code_challenge="));
        assert!(res.auth_url.contains("response_type=code"));
        assert!(!res.state.is_empty());
        assert!(res.callback_port >= 8891);
        let flows = svc.pending_flows.lock().await;
        assert!(flows.contains_key(&res.state));
    }
}
