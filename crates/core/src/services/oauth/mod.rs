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
mod tokens;
mod trovo;

#[cfg(test)]
mod tests;

pub use config::OAuthConfig;
pub use device::OAuthDeviceFlowStart;
pub use flow::OAuthFlowResult;
pub use loopback::{OAuthCallback, OAuthCallbackServer};
pub use provider::OAuthProvider;
pub use tokens::{
    OAuthCompleteResult, OAuthRefreshOutcome, OAuthTokens, OAuthUserInfo, TwitchUser,
    YouTubeChannel,
};
pub use trovo::TrovoUser;

use crate::errors::CoreError;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

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
    pub fn new(config: OAuthConfig) -> Self {
        Self {
            config: Arc::new(Mutex::new(config)),
            pending_flows: Arc::new(Mutex::new(HashMap::new())),
            http_client: reqwest::Client::new(),
            refresh_lock: tokio::sync::Mutex::new(()),
            #[cfg(test)]
            provider_overrides: std::sync::Mutex::new(HashMap::new()),
        }
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

    pub async fn update_config(&self, config: OAuthConfig) {
        let mut current = self.config.lock().await;
        *current = config;
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

    /// One snapshot of every provider's configured state — single source
    /// of truth for the HTTP config endpoint AND `spiritstream-cli
    /// oauth config`, so the two transports can't drift.
    pub async fn configured_flags(&self) -> OAuthConfiguredFlags {
        let config = self.config.lock().await;
        OAuthConfiguredFlags {
            twitch: config.has_twitch(),
            youtube: config.has_youtube(),
            kick: config.has_kick(),
            facebook: config.has_facebook(),
            trovo: config.has_trovo(),
        }
    }
}

/// Per-provider "are real credentials present in this build/env" flags.
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthConfiguredFlags {
    pub twitch: bool,
    pub youtube: bool,
    pub kick: bool,
    pub facebook: bool,
    pub trovo: bool,
}
