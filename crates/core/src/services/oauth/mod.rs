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
mod flow;
mod loopback;
mod pkce;
mod provider;
mod tokens;

#[cfg(test)]
mod tests;

pub use config::OAuthConfig;
pub use flow::OAuthFlowResult;
pub use loopback::{OAuthCallback, OAuthCallbackServer};
pub use provider::OAuthProvider;
pub use tokens::{
    OAuthCompleteResult, OAuthRefreshOutcome, OAuthTokens, OAuthUserInfo, TwitchUser,
    YouTubeChannel,
};

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
}

impl OAuthService {
    pub fn new(config: OAuthConfig) -> Self {
        Self {
            config: Arc::new(Mutex::new(config)),
            pending_flows: Arc::new(Mutex::new(HashMap::new())),
            http_client: reqwest::Client::new(),
            refresh_lock: tokio::sync::Mutex::new(()),
        }
    }

    pub async fn update_config(&self, config: OAuthConfig) {
        let mut current = self.config.lock().await;
        *current = config;
    }

    pub async fn get_config(&self) -> OAuthConfig {
        self.config.lock().await.clone()
    }

    /// Always true with embedded client IDs.
    pub async fn is_configured(&self, provider: &str) -> bool {
        matches!(provider, "twitch" | "youtube" | "kick" | "facebook")
    }
}
