use crate::errors::CoreError;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use super::provider::OAuthProvider;
use super::{network, unknown_provider};

/// OAuth tokens returned from the token exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// Per-provider outcome of `OAuthService::refresh_profile_tokens`. Lets the
/// orchestrator (and transports) react to refresh failures without comparing
/// before/after `expires_at` themselves.
#[derive(Debug, Default, Clone)]
pub struct OAuthRefreshOutcome {
    /// Providers whose access token was rotated and written into the profile.
    /// Caller is responsible for persisting the profile after refresh.
    pub refreshed: Vec<String>,
    /// Providers whose token was inside the leeway window but refresh failed
    /// (network blip, provider 4xx). Transport-level UX uses this list to
    /// emit `oauth_token_expired` so users can re-auth without blocking
    /// activation.
    pub failed: Vec<String>,
}

/// User info returned after successful OAuth.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthUserInfo {
    pub provider: String,
    pub user_id: String,
    pub username: String,
    pub display_name: String,
}

/// Complete OAuth result with tokens and user info.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthCompleteResult {
    pub tokens: OAuthTokens,
    pub user_info: OAuthUserInfo,
}

/// Twitch user info from token validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwitchUser {
    pub id: String,
    pub login: String,
    pub display_name: String,
}

/// YouTube channel info.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YouTubeChannel {
    pub id: String,
    pub title: String,
}

/// Kick user info from `GET /public/v1/users` (the "me" endpoint —
/// returns the user identified by the bearer token). `user_id` is the
/// numeric broadcaster id Kick's REST `POST /chat` expects as
/// `broadcaster_user_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct KickUser {
    pub user_id: u64,
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub profile_picture: Option<String>,
}

impl super::OAuthService {
    /// Pre-flight check: does this token expire within `leeway_secs` of now?
    /// Callers about to use an access token should call this and request a
    /// refresh when it returns `true` to avoid an in-flight 401. Expiry
    /// handling lives in core (this method), not in every consumer.
    ///
    /// `expires_at` is Unix epoch seconds; `0` means "no expiry recorded"
    /// (treated as not-expiring so we don't churn refresh on freshly-set
    /// profiles that haven't been issued a real expiry yet).
    pub fn token_needs_refresh(&self, expires_at: i64, leeway_secs: i64) -> bool {
        if expires_at == 0 {
            return false;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
            .unwrap_or(0);
        expires_at <= now.saturating_add(leeway_secs)
    }

    /// Refresh `access_token` only when it's about to expire. Idempotent —
    /// returns `Ok(None)` if no refresh was needed, `Ok(Some(new_tokens))`
    /// when the access token was actually rotated, or `Err(CoreError)` on
    /// failure. Callers persist the new tokens themselves so the storage
    /// boundary (encrypted-at-rest) stays in one place.
    pub async fn refresh_if_expiring(
        &self,
        provider_name: &str,
        refresh_token: &str,
        expires_at: i64,
        leeway_secs: i64,
    ) -> Result<Option<OAuthTokens>, CoreError> {
        if !self.token_needs_refresh(expires_at, leeway_secs) {
            return Ok(None);
        }
        if refresh_token.is_empty() {
            // No refresh token to spend — surface as Unauthorized so the
            // transport can prompt the user to log in again.
            return Err(CoreError::Unauthorized);
        }
        let tokens = self.refresh_token(provider_name, refresh_token).await?;
        Ok(Some(tokens))
    }

    /// Iterate every supported provider on the given profile, refresh any
    /// access token that's inside the leeway window, and mutate the profile
    /// in place with the rotated credentials. The caller is responsible for
    /// persisting the profile when `outcome.refreshed` is non-empty.
    ///
    /// Per-provider refresh failures are logged + surveilled but never abort
    /// the loop: a Twitch network blip must not block YouTube refresh. The
    /// `failed` list carries the providers the transport should surface as
    /// "needs re-auth" — the orchestration owner decides how (bus event,
    /// CLI exit code, etc).
    pub async fn refresh_profile_tokens(
        &self,
        profile: &mut crate::models::Profile,
        leeway_secs: i64,
        surveillance: Option<&Arc<crate::services::AuthSurveillanceService>>,
    ) -> Result<OAuthRefreshOutcome, CoreError> {
        let mut outcome = OAuthRefreshOutcome::default();

        for provider in ["twitch", "youtube", "kick"] {
            let (access, refresh, expires_at) = match provider {
                "twitch" => (
                    profile.settings.oauth.twitch.access_token.clone(),
                    profile.settings.oauth.twitch.refresh_token.clone(),
                    profile.settings.oauth.twitch.expires_at,
                ),
                "youtube" => (
                    profile.settings.oauth.youtube.access_token.clone(),
                    profile.settings.oauth.youtube.refresh_token.clone(),
                    profile.settings.oauth.youtube.expires_at,
                ),
                "kick" => (
                    profile.settings.oauth.kick.access_token.clone(),
                    profile.settings.oauth.kick.refresh_token.clone(),
                    profile.settings.oauth.kick.expires_at,
                ),
                _ => continue,
            };
            if access.is_empty() || refresh.is_empty() {
                continue;
            }
            match self
                .refresh_if_expiring(provider, &refresh, expires_at, leeway_secs)
                .await
            {
                Ok(None) => {}
                Ok(Some(tokens)) => {
                    if let Some(s) = surveillance {
                        let _ = s.record_oauth_refresh(provider, true).await;
                    }
                    let now_secs = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
                        .unwrap_or(0);
                    let expires_in =
                        i64::try_from(tokens.expires_in.unwrap_or(0)).unwrap_or(i64::MAX);
                    let new_expires = now_secs.saturating_add(expires_in);
                    match provider {
                        "twitch" => {
                            profile.settings.oauth.twitch.access_token = tokens.access_token;
                            if let Some(rt) = tokens.refresh_token {
                                profile.settings.oauth.twitch.refresh_token = rt;
                            }
                            profile.settings.oauth.twitch.expires_at = new_expires;
                        }
                        "youtube" => {
                            profile.settings.oauth.youtube.access_token = tokens.access_token;
                            if let Some(rt) = tokens.refresh_token {
                                profile.settings.oauth.youtube.refresh_token = rt;
                            }
                            profile.settings.oauth.youtube.expires_at = new_expires;
                        }
                        "kick" => {
                            profile.settings.oauth.kick.access_token = tokens.access_token;
                            if let Some(rt) = tokens.refresh_token {
                                profile.settings.oauth.kick.refresh_token = rt;
                            }
                            profile.settings.oauth.kick.expires_at = new_expires;
                        }
                        _ => {}
                    }
                    outcome.refreshed.push(provider.to_string());
                }
                Err(e) => {
                    if let Some(s) = surveillance {
                        let _ = s.record_oauth_refresh(provider, false).await;
                    }
                    log::warn!("OAuth refresh for {provider} failed during activate: {e}");
                    outcome.failed.push(provider.to_string());
                }
            }
        }

        Ok(outcome)
    }

    /// Refresh an access token using a refresh token.
    pub async fn refresh_token(
        &self,
        provider_name: &str,
        refresh_token: &str,
    ) -> Result<OAuthTokens, CoreError> {
        let config = self.config.lock().await;

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
            _ => return Err(unknown_provider(provider_name)),
        };

        drop(config);

        let mut params = HashMap::new();
        params.insert("client_id", client_id.as_str());
        params.insert("refresh_token", refresh_token);
        params.insert("grant_type", "refresh_token");

        // Google Desktop apps require client_secret even for refresh (non-standard).
        let client_secret_owned = client_secret;
        if let Some(ref secret) = client_secret_owned {
            params.insert("client_secret", secret.as_str());
        }

        info!("Refreshing {} access token", provider_name);

        let response = self
            .http_client
            .post(provider.token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| network(format!("Token refresh request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            // Drop the body without logging — providers echo submitted form
            // fields (refresh_token, client_secret) in error JSON.
            let _ = response.text().await;
            error!("Token refresh failed: {}", status);
            // Differentiate so callers know whether to prompt re-login
            // (4xx — refresh token revoked / bad_grant) or retry after
            // backoff (5xx — provider transient outage).
            return if status.is_client_error() {
                Err(CoreError::Unauthorized)
            } else {
                Err(CoreError::NetworkError {
                    detail: format!("Token refresh failed: {status}"),
                })
            };
        }

        let tokens: OAuthTokens = response.json().await.map_err(|e| CoreError::Internal {
            context: format!("Failed to parse token response: {e}"),
        })?;

        info!("Successfully refreshed {} tokens", provider_name);
        Ok(tokens)
    }

    /// Fetch Twitch user info using an access token.
    pub async fn fetch_twitch_user(&self, access_token: &str) -> Result<TwitchUser, CoreError> {
        let config = self.config.lock().await;
        let client_id = config.get_twitch_client_id();
        drop(config);

        let response = self
            .http_client
            .get("https://api.twitch.tv/helix/users")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Client-Id", client_id)
            .send()
            .await
            .map_err(|e| network(format!("Twitch user request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let _ = response.text().await;
            error!("Twitch user fetch failed: {}", status);
            return Err(CoreError::Unauthorized);
        }

        #[derive(Deserialize)]
        struct TwitchResponse {
            data: Vec<TwitchUser>,
        }

        let data: TwitchResponse = response.json().await.map_err(|e| CoreError::Internal {
            context: format!("Failed to parse Twitch response: {e}"),
        })?;

        data.data
            .into_iter()
            .next()
            .ok_or_else(|| CoreError::Internal {
                context: "No user data in Twitch response".into(),
            })
    }

    /// Fetch YouTube channel info using an access token.
    pub async fn fetch_youtube_channel(
        &self,
        access_token: &str,
    ) -> Result<YouTubeChannel, CoreError> {
        let response = self
            .http_client
            .get("https://www.googleapis.com/youtube/v3/channels")
            .query(&[("part", "snippet"), ("mine", "true")])
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
            .map_err(|e| network(format!("YouTube channel request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let _ = response.text().await;
            error!("YouTube channel fetch failed: {}", status);
            return Err(CoreError::Unauthorized);
        }

        #[derive(Deserialize)]
        struct YouTubeResponse {
            items: Option<Vec<YouTubeItem>>,
        }

        #[derive(Deserialize)]
        struct YouTubeItem {
            id: String,
            snippet: YouTubeSnippet,
        }

        #[derive(Deserialize)]
        struct YouTubeSnippet {
            title: String,
        }

        let data: YouTubeResponse = response.json().await.map_err(|e| CoreError::Internal {
            context: format!("Failed to parse YouTube response: {e}"),
        })?;

        let item = data
            .items
            .and_then(|items| items.into_iter().next())
            .ok_or_else(|| CoreError::Internal {
                context: "No channel data in YouTube response".into(),
            })?;

        Ok(YouTubeChannel {
            id: item.id,
            title: item.snippet.title,
        })
    }

    /// Validate a Twitch token and get user info (alias for fetch_twitch_user).
    pub async fn validate_twitch_token(&self, access_token: &str) -> Result<TwitchUser, CoreError> {
        self.fetch_twitch_user(access_token).await
    }

    /// Fetch the bearer-identified Kick user. Kick exposes a `users` REST
    /// endpoint that returns the broadcaster id needed to address
    /// `POST /public/v1/chat`.
    pub async fn fetch_kick_user(&self, access_token: &str) -> Result<KickUser, CoreError> {
        let response = self
            .http_client
            .get("https://api.kick.com/public/v1/users")
            .header("Authorization", format!("Bearer {access_token}"))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| network(format!("Kick user request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let _ = response.text().await;
            error!("Kick user fetch failed: {}", status);
            return Err(CoreError::Unauthorized);
        }

        #[derive(Deserialize)]
        struct KickEnvelope {
            data: Vec<KickUser>,
        }

        let envelope: KickEnvelope = response.json().await.map_err(|e| CoreError::Internal {
            context: format!("Failed to parse Kick user response: {e}"),
        })?;

        envelope
            .data
            .into_iter()
            .next()
            .ok_or_else(|| CoreError::Internal {
                context: "No user data in Kick response".into(),
            })
    }

    /// Revoke a token (best effort — not all providers support this).
    pub async fn revoke_token(&self, provider_name: &str, token: &str) -> Result<(), CoreError> {
        match provider_name {
            "twitch" => {
                let config = self.config.lock().await;
                let client_id = config.get_twitch_client_id();
                drop(config);

                let response = self
                    .http_client
                    .post("https://id.twitch.tv/oauth2/revoke")
                    .form(&[("client_id", client_id.as_str()), ("token", token)])
                    .send()
                    .await
                    .map_err(|e| network(format!("Token revoke request failed: {e}")))?;

                if !response.status().is_success() {
                    warn!("Twitch token revocation returned non-success status");
                }
                Ok(())
            }
            "youtube" => {
                let response = self
                    .http_client
                    .post("https://oauth2.googleapis.com/revoke")
                    .form(&[("token", token)])
                    .send()
                    .await
                    .map_err(|e| network(format!("Token revoke request failed: {e}")))?;

                if !response.status().is_success() {
                    warn!("YouTube token revocation returned non-success status");
                }
                Ok(())
            }
            "kick" => {
                let config = self.config.lock().await;
                let client_id = config.get_kick_client_id();
                drop(config);

                // Kick's id.kick.com supports token revocation per RFC 7009.
                let response = self
                    .http_client
                    .post("https://id.kick.com/oauth/revoke")
                    .form(&[("client_id", client_id.as_str()), ("token", token)])
                    .send()
                    .await
                    .map_err(|e| network(format!("Token revoke request failed: {e}")))?;

                if !response.status().is_success() {
                    warn!("Kick token revocation returned non-success status");
                }
                Ok(())
            }
            _ => Err(unknown_provider(provider_name)),
        }
    }
}
