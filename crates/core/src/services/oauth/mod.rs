//! OAuth 2.0 authentication for Twitch and YouTube.
//!
//! Split by concern:
//! - `provider`: Twitch/YouTube endpoint metadata.
//! - `config`: user-overridable client IDs/secrets with env fallback.
//! - `pkce`: PKCE verifier/challenge generator.
//! - `flow`: auth-code + implicit flows; `start_flow` / `exchange_code` /
//!   `complete_flow` impls on `OAuthService`.
//! - `tokens`: refresh, profile-wide refresh, user-info fetch, revocation.
//! - `loopback`: TCP callback server bound on localhost for the loopback
//!   redirect URI.

mod config;
mod device;
mod flow;
mod loopback;
mod pkce;
mod provider;
mod setup;
mod tokens;
mod trovo;

#[cfg(test)]
mod tests;

pub use config::OAuthConfig;
pub(crate) use config::is_real as credential_is_real;
pub use device::OAuthDeviceFlowStart;
pub use flow::OAuthFlowResult;
pub use loopback::{OAuthCallback, OAuthCallbackServer};
pub use provider::{OAuthProvider, FACEBOOK_GRAPH_VERSION};
pub use setup::{OAuthConsoleField, OAuthProviderSetup};
pub use tokens::{
    OAuthCompleteResult, OAuthRefreshOutcome, OAuthTokens, OAuthUserInfo, TwitchUser,
    YouTubeChannel,
};
pub use trovo::TrovoUser;

use crate::errors::CoreError;
use crate::traits::SecretStore;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// SecretStore location for user-entered client credentials (the
/// "explicit override" tier of the resolution chain). One JSON blob —
/// ids and secrets travel together so a partial write can't leave a
/// half-updated provider.
const CRED_NAMESPACE: &str = "oauth";
const CRED_KEY: &str = "client_credentials";

pub(super) fn unknown_provider(name: &str) -> CoreError {
    CoreError::NotImplemented {
        feature: format!("Unknown provider: {name}"),
    }
}

pub(super) fn network(detail: impl Into<String>) -> CoreError {
    CoreError::NetworkError {
        detail: detail.into(),
    }
}

/// OAuth service for handling authentication flows. Method impls live
/// in `flow.rs` (start/exchange/complete) and `tokens.rs` (refresh /
/// fetch / revoke); the struct itself plus the lightweight config
/// accessors stay here so submods can `impl super::OAuthService`.
pub struct OAuthService {
    pub(in crate::services::oauth) config: Arc<Mutex<OAuthConfig>>,
    /// At-rest home for user-entered client credentials, so setup done
    /// in the UI survives restarts. Loaded once via `load_persisted`
    /// at startup; written through on every credential change.
    pub(in crate::services::oauth) secret_store: Arc<dyn SecretStore>,
    pub(in crate::services::oauth) pending_flows:
        Arc<Mutex<HashMap<String, flow::PendingOAuthFlow>>>,
    pub(in crate::services::oauth) http_client: reqwest::Client,
    /// Serialises token refreshes. Providers that rotate refresh tokens
    /// on use (Twitch; Google with rotation enabled) hand out exactly
    /// one valid refresh token at a time — two concurrent refresh paths
    /// (activation, chat lifecycle, the standalone refresh endpoint)
    /// spending the same token meant the loser got `invalid_grant` and
    /// could clobber the winner's freshly-rotated credentials, leaving
    /// the account unauthenticated mid-stream. Global (not per-profile)
    /// on purpose: this is a single-active-profile app and the lock is
    /// only held across one HTTP round-trip.
    pub(in crate::services::oauth) refresh_lock: tokio::sync::Mutex<()>,
    /// Test-only endpoint rebasing (wiremock) — same philosophy as
    /// `ChatEndpoints::for_mock`. Empty in production.
    #[cfg(test)]
    pub(in crate::services::oauth) provider_overrides:
        std::sync::Mutex<HashMap<String, OAuthProvider>>,
}

impl OAuthService {
    pub fn new(config: OAuthConfig, secret_store: Arc<dyn SecretStore>) -> Self {
        Self {
            config: Arc::new(Mutex::new(config)),
            secret_store,
            pending_flows: Arc::new(Mutex::new(HashMap::new())),
            http_client: reqwest::Client::new(),
            refresh_lock: tokio::sync::Mutex::new(()),
            #[cfg(test)]
            provider_overrides: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Unit-test constructor: file-backed secret store in a fresh temp
    /// dir (leaked so the store outlives the test body — the OS reaps
    /// the tmpdir). Persistence-roundtrip tests build their own store
    /// over a shared dir instead.
    #[cfg(test)]
    pub(in crate::services::oauth) fn new_for_tests(config: OAuthConfig) -> Self {
        let dir = tempfile::tempdir().expect("tempdir for oauth test store");
        let store = crate::services::build_secret_store(dir.path(), Some("file"))
            .expect("file secret store for oauth tests");
        std::mem::forget(dir);
        Self::new(config, store)
    }

    /// Load the credential overrides saved by `set_provider_credentials`
    /// / `update_config` back into memory. Transports call this once at
    /// startup, right after registry build — without it, setup done in
    /// the UI would silently vanish on restart.
    pub async fn load_persisted(&self) -> Result<(), CoreError> {
        let Some(bytes) = self.secret_store.get(CRED_NAMESPACE, CRED_KEY).await? else {
            return Ok(());
        };
        let stored: OAuthConfig =
            serde_json::from_slice(&bytes).map_err(|e| CoreError::Internal {
                context: format!("stored OAuth client credentials are unreadable: {e}"),
            })?;
        *self.config.lock().await = stored;
        Ok(())
    }

    async fn persist(&self, config: &OAuthConfig) -> Result<(), CoreError> {
        let bytes = serde_json::to_vec(config).map_err(|e| CoreError::Internal {
            context: format!("serialize OAuth client credentials: {e}"),
        })?;
        self.secret_store.put(CRED_NAMESPACE, CRED_KEY, &bytes).await
    }

    /// Set (or clear, with `None`) one provider's client credentials and
    /// persist the result. This is the backend behind the in-app
    /// "Set up sign-in" form — the user pastes credentials from the
    /// provider's developer portal and never touches an env file.
    /// Empty/whitespace inputs clear the override.
    pub async fn set_provider_credentials(
        &self,
        provider: &str,
        client_id: Option<String>,
        client_secret: Option<String>,
    ) -> Result<(), CoreError> {
        let normalize = |v: Option<String>| -> Option<String> {
            v.and_then(|s| {
                let trimmed = s.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            })
        };
        let id = normalize(client_id);
        let secret = normalize(client_secret);

        let mut config = self.config.lock().await;
        match provider {
            "twitch" => {
                config.twitch_client_id = id;
                config.twitch_client_secret = secret;
            }
            "youtube" => {
                config.youtube_client_id = id;
                config.youtube_client_secret = secret;
            }
            "kick" => {
                config.kick_client_id = id;
                config.kick_client_secret = secret;
            }
            "facebook" => {
                config.facebook_client_id = id;
                config.facebook_client_secret = secret;
            }
            "trovo" => {
                config.trovo_client_id = id;
                config.trovo_client_secret = secret;
            }
            other => return Err(unknown_provider(other)),
        }
        self.persist(&config).await
    }

    /// Resolve a provider's endpoint set. The single lookup every flow
    /// (loopback, device, refresh) goes through, so tests can rebase
    /// one provider onto wiremock and exercise the real request code.
    pub(in crate::services::oauth) fn provider_for(
        &self,
        name: &str,
    ) -> Result<OAuthProvider, CoreError> {
        #[cfg(test)]
        {
            if let Ok(overrides) = self.provider_overrides.lock() {
                if let Some(p) = overrides.get(name) {
                    return Ok(p.clone());
                }
            }
        }
        match name {
            "twitch" => Ok(OAuthProvider::twitch()),
            "youtube" => Ok(OAuthProvider::youtube()),
            "kick" => Ok(OAuthProvider::kick()),
            "facebook" => Ok(OAuthProvider::facebook()),
            "trovo" => Ok(OAuthProvider::trovo()),
            _ => Err(unknown_provider(name)),
        }
    }

    #[cfg(test)]
    pub(in crate::services::oauth) fn override_provider(&self, provider: OAuthProvider) {
        self.provider_overrides
            .lock()
            .expect("override lock")
            .insert(provider.name.clone(), provider);
    }

    /// Full-replace of the credential overrides (CLI `oauth config set`
    /// and the admin HTTP PUT). Persists like the per-provider setter.
    pub async fn update_config(&self, config: OAuthConfig) -> Result<(), CoreError> {
        let mut current = self.config.lock().await;
        *current = config;
        self.persist(&current).await
    }

    pub async fn get_config(&self) -> OAuthConfig {
        self.config.lock().await.clone()
    }

    /// True only when the provider's resolved credentials are REAL
    /// (not the embedded placeholders) — see `OAuthConfig::has_*`.
    /// This is what gates sign-in buttons; lying `true` here was the
    /// root of the "Login with Twitch opens a dead page" bug.
    pub async fn is_configured(&self, provider: &str) -> bool {
        let config = self.config.lock().await;
        match provider {
            "twitch" => config.has_twitch(),
            "youtube" => config.has_youtube(),
            "kick" => config.has_kick(),
            "facebook" => config.has_facebook(),
            "trovo" => config.has_trovo(),
            _ => false,
        }
    }

    /// One snapshot of every provider's setup state — single source of
    /// truth for the HTTP config endpoint AND `spiritstream-cli oauth
    /// config`, so the two transports can't drift. Carries everything
    /// the in-app setup form needs (which fields to show, where to
    /// register, what override is active) so the frontend holds zero
    /// provider knowledge. Secret values never appear here.
    /// `channel_hints` maps provider → the channel/username the user
    /// already entered for that platform (read from the active profile
    /// by the transport). It's woven into the guided console setup so
    /// the suggested app name is pre-filled — the user never has to
    /// invent one.
    pub async fn provider_summaries(
        &self,
        channel_hints: &HashMap<String, String>,
    ) -> Vec<OAuthProviderSummary> {
        let config = self.config.lock().await;
        let summary = |provider: &str, configured: bool, needs_secret: bool, id: &Option<String>| {
            OAuthProviderSummary {
                provider: provider.to_string(),
                configured,
                needs_secret,
                override_client_id: id.clone(),
                registration_url: provider::registration_url(provider).to_string(),
                setup: setup::console_setup(provider, channel_hints.get(provider).map(String::as_str)),
            }
        };
        vec![
            // Twitch is a public client (Device Code Flow): id only.
            summary("twitch", config.has_twitch(), false, &config.twitch_client_id),
            summary("youtube", config.has_youtube(), true, &config.youtube_client_id),
            summary("kick", config.has_kick(), true, &config.kick_client_id),
            summary("facebook", config.has_facebook(), true, &config.facebook_client_id),
            summary("trovo", config.has_trovo(), true, &config.trovo_client_id),
        ]
    }
}

/// Per-provider setup state for the config surface. `configured` gates
/// the sign-in buttons; the rest drives the in-app credentials form.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthProviderSummary {
    pub provider: String,
    /// Real credentials present (override, env, or release-embedded).
    pub configured: bool,
    /// Whether this provider's token exchange requires a client secret.
    pub needs_secret: bool,
    /// The stored client-id override, when the user has entered one.
    pub override_client_id: Option<String>,
    /// The provider's developer-portal page where the app is registered.
    pub registration_url: String,
    /// Pre-filled, copy-pasteable console fields + steps so the user
    /// never has to figure out what to type into the developer portal.
    pub setup: OAuthProviderSetup,
}
