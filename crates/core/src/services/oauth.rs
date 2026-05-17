//! OAuth 2.0 authentication service for Twitch and YouTube
//!
//! This module handles the OAuth flow for desktop applications using the
//! loopback redirect method (localhost callback) with PKCE for security.

use crate::errors::CoreError;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use log::{error, info, warn};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{oneshot, Mutex};

fn unknown_provider(name: &str) -> CoreError {
    CoreError::NotImplemented {
        feature: format!("Unknown provider: {name}"),
    }
}

fn network(detail: impl Into<String>) -> CoreError {
    CoreError::NetworkError {
        detail: detail.into(),
    }
}

// ============================================================================
// Embedded OAuth Client IDs (PKCE flow - no secrets needed)
// ============================================================================

/// Twitch OAuth Client ID (replace with your registered app's client ID)
const TWITCH_CLIENT_ID: &str = "TWITCH_CLIENT_ID_PLACEHOLDER";

/// YouTube/Google OAuth Client ID (replace with your registered app's client ID)
const YOUTUBE_CLIENT_ID: &str = "YOUTUBE_CLIENT_ID_PLACEHOLDER";

// ============================================================================
// OAuth Provider Configuration
// ============================================================================

/// OAuth provider configuration
#[derive(Debug, Clone)]
pub struct OAuthProvider {
    pub name: &'static str,
    pub auth_url: &'static str,
    pub token_url: &'static str,
    pub scopes: Vec<&'static str>,
}

impl OAuthProvider {
    /// Twitch OAuth configuration
    pub fn twitch() -> Self {
        Self {
            name: "twitch",
            auth_url: "https://id.twitch.tv/oauth2/authorize",
            token_url: "https://id.twitch.tv/oauth2/token",
            scopes: vec!["chat:read", "chat:edit"],
        }
    }

    /// YouTube/Google OAuth configuration
    pub fn youtube() -> Self {
        Self {
            name: "youtube",
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth",
            token_url: "https://oauth2.googleapis.com/token",
            scopes: vec!["https://www.googleapis.com/auth/youtube.force-ssl"],
        }
    }
}

// ============================================================================
// Data Structures
// ============================================================================

/// OAuth tokens returned from the token exchange
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

/// OAuth configuration (for optional user-provided credentials)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OAuthConfig {
    /// Optional user-provided Twitch OAuth client ID (overrides embedded)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twitch_client_id: Option<String>,
    /// Optional user-provided Twitch OAuth client secret (for confidential flows)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twitch_client_secret: Option<String>,
    /// Optional user-provided YouTube/Google OAuth client ID (overrides embedded)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub youtube_client_id: Option<String>,
    /// Optional user-provided YouTube/Google OAuth client secret (for confidential flows)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub youtube_client_secret: Option<String>,
}

impl OAuthConfig {
    /// Check if Twitch OAuth is available (always true with embedded ID)
    pub fn has_twitch(&self) -> bool {
        true // Embedded client ID is always available
    }

    /// Check if YouTube OAuth is available (always true with embedded ID)
    pub fn has_youtube(&self) -> bool {
        true // Embedded client ID is always available
    }

    /// Get Twitch client ID (user-provided, env override, or embedded)
    pub fn get_twitch_client_id(&self) -> String {
        if let Some(value) = self.twitch_client_id.as_deref() {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }

        if let Ok(value) = std::env::var("SPIRITSTREAM_TWITCH_CLIENT_ID") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }

        TWITCH_CLIENT_ID.to_string()
    }

    /// Get Twitch client secret (user-provided or env override)
    pub fn get_twitch_client_secret(&self) -> Option<String> {
        if let Some(value) = self.twitch_client_secret.as_deref() {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }

        if let Ok(value) = std::env::var("SPIRITSTREAM_TWITCH_CLIENT_SECRET") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }

        None
    }

    /// Get YouTube client ID (user-provided, env override, or embedded)
    pub fn get_youtube_client_id(&self) -> String {
        if let Some(value) = self.youtube_client_id.as_deref() {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }

        if let Ok(value) = std::env::var("SPIRITSTREAM_YOUTUBE_CLIENT_ID") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }

        YOUTUBE_CLIENT_ID.to_string()
    }

    /// Get YouTube client secret (user-provided or env override)
    /// Required for Google Desktop apps even with PKCE
    pub fn get_youtube_client_secret(&self) -> Option<String> {
        if let Some(value) = self.youtube_client_secret.as_deref() {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }

        if let Ok(value) = std::env::var("SPIRITSTREAM_YOUTUBE_CLIENT_SECRET") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }

        None
    }
}

/// Result of initiating an OAuth flow. ts-rs-exported so the
/// `@spiritstream/types` package is the single source of truth on
/// the wire shape — no parallel hand-written TS file.
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
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
struct PendingOAuthFlow {
    provider: String,
    state: String,
    code_verifier: String,
    redirect_uri: String,
    created_at: Instant,
}

/// User info returned after successful OAuth
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthUserInfo {
    pub provider: String,
    pub user_id: String,
    pub username: String,
    pub display_name: String,
}

/// Complete OAuth result with tokens and user info
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthCompleteResult {
    pub tokens: OAuthTokens,
    pub user_info: OAuthUserInfo,
}

/// Twitch user info from token validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwitchUser {
    pub id: String,
    pub login: String,
    pub display_name: String,
}

/// YouTube channel info
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YouTubeChannel {
    pub id: String,
    pub title: String,
}

// ============================================================================
// PKCE Implementation
// ============================================================================

/// Generate a PKCE code verifier and challenge pair
fn generate_pkce_pair() -> (String, String) {
    // Generate 32 random bytes for the code verifier
    let mut rng = rand::thread_rng();
    let random_bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();

    // Base64url encode to get the code_verifier
    let code_verifier = URL_SAFE_NO_PAD.encode(&random_bytes);

    // SHA256 hash the verifier, then base64url encode for the challenge
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let hash = hasher.finalize();
    let code_challenge = URL_SAFE_NO_PAD.encode(hash);

    (code_verifier, code_challenge)
}

// ============================================================================
// OAuth Service
// ============================================================================

/// OAuth service for handling authentication flows
pub struct OAuthService {
    config: Arc<Mutex<OAuthConfig>>,
    pending_flows: Arc<Mutex<HashMap<String, PendingOAuthFlow>>>,
    http_client: reqwest::Client,
}

impl OAuthService {
    /// Create a new OAuth service
    pub fn new(config: OAuthConfig) -> Self {
        Self {
            config: Arc::new(Mutex::new(config)),
            pending_flows: Arc::new(Mutex::new(HashMap::new())),
            http_client: reqwest::Client::new(),
        }
    }

    /// Update the OAuth configuration
    pub async fn update_config(&self, config: OAuthConfig) {
        let mut current = self.config.lock().await;
        *current = config;
    }

    /// Get the current OAuth configuration
    pub async fn get_config(&self) -> OAuthConfig {
        self.config.lock().await.clone()
    }

    /// Check if a provider is configured (always true with embedded IDs)
    pub async fn is_configured(&self, provider: &str) -> bool {
        matches!(provider, "twitch" | "youtube")
    }

    /// Find an available port for the callback server
    /// Uses a small set of fixed ports so that redirect URIs can be pre-registered
    /// with OAuth providers (Twitch requires exact match including port)
    fn find_available_port() -> Result<u16, CoreError> {
        // Preferred fixed ports for OAuth callbacks -- register these in provider consoles
        const PREFERRED_PORTS: &[u16] = &[8891, 8892, 8893, 8894, 8895];

        for &port in PREFERRED_PORTS {
            if TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok() {
                return Ok(port);
            }
        }

        // Fallback to ephemeral range if all preferred ports are busy
        for port in 49152..49162 {
            if TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok() {
                return Ok(port);
            }
        }
        Err(CoreError::Internal {
            context: "No available port found for OAuth callback".into(),
        })
    }

    /// Clean up expired pending flows (older than 10 minutes)
    async fn cleanup_expired_flows(&self) {
        let mut flows = self.pending_flows.lock().await;
        let now = Instant::now();
        flows.retain(|_, flow| now.duration_since(flow.created_at) < Duration::from_secs(600));
    }

    /// Build the authorization URL (PKCE optional for code flow)
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

        // Add PKCE params only for authorization code flow
        if let Some(challenge) = code_challenge {
            params.push(("code_challenge", challenge));
            params.push(("code_challenge_method", "S256"));
        }

        // YouTube/Google requires additional params
        if provider.name == "youtube" {
            params.push(("access_type", "offline"));
            params.push(("prompt", "consent"));
        }

        // Twitch requires force_verify for re-auth
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
        // Clean up any expired flows
        self.cleanup_expired_flows().await;

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
            _ => return Err(unknown_provider(provider_name)),
        };

        drop(config); // Release lock

        // Find an available port
        let port = Self::find_available_port()?;
        let redirect_uri = format!("http://localhost:{}/oauth/callback", port);

        // Decide which flow to use
        let (use_implicit, use_pkce) = match provider_name {
            // Twitch requires client secret for auth-code token exchange.
            // If no secret is configured, fall back to implicit flow.
            "twitch" => (client_secret.is_none(), false),
            // YouTube uses auth code + PKCE.
            _ => (false, true),
        };

        // Generate PKCE pair only when needed
        let (code_verifier, code_challenge) = if use_pkce {
            let (v, c) = generate_pkce_pair();
            (v, Some(c))
        } else {
            (String::new(), None)
        };

        // Generate a random state for CSRF protection
        let state = uuid::Uuid::new_v4().to_string();

        // Build the auth URL
        let response_type = if use_implicit { "token" } else { "code" };
        let auth_url = Self::build_auth_url(
            &provider,
            &client_id,
            &redirect_uri,
            &state,
            response_type,
            code_challenge.as_deref(),
        );

        // Debug: log provider + client_id hint (avoid full ID in logs)
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

        // Store the pending flow
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

        let flow_label = if use_implicit {
            "implicit"
        } else if use_pkce {
            "PKCE"
        } else {
            "auth code"
        };
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

    /// Exchange an authorization code for tokens (PKCE flow)
    pub async fn exchange_code(
        &self,
        provider_name: &str,
        code: &str,
        state: &str,
    ) -> Result<OAuthTokens, CoreError> {
        // Find and remove the pending flow
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

        // Verify provider matches
        if pending_flow.provider != provider_name {
            return Err(CoreError::Internal {
                context: "Provider mismatch in OAuth flow".into(),
            });
        }

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
            _ => return Err(unknown_provider(provider_name)),
        };

        drop(config);

        // Build token exchange request with PKCE code_verifier
        let mut params = HashMap::new();
        params.insert("client_id", client_id.as_str());
        params.insert("code", code);
        if !pending_flow.code_verifier.is_empty() {
            params.insert("code_verifier", pending_flow.code_verifier.as_str());
        }
        params.insert("grant_type", "authorization_code");
        params.insert("redirect_uri", pending_flow.redirect_uri.as_str());

        // Google Desktop apps require client_secret even with PKCE (non-standard)
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

        for provider in ["twitch", "youtube"] {
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

    /// Refresh an access token using a refresh token
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
            _ => return Err(unknown_provider(provider_name)),
        };

        drop(config);

        let mut params = HashMap::new();
        params.insert("client_id", client_id.as_str());
        params.insert("refresh_token", refresh_token);
        params.insert("grant_type", "refresh_token");

        // Google Desktop apps require client_secret even for refresh (non-standard)
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

    /// Fetch Twitch user info using an access token
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

    /// Fetch YouTube channel info using an access token
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

    /// Complete the OAuth flow: exchange code, fetch user info
    pub async fn complete_flow(
        &self,
        provider_name: &str,
        code: &str,
        state: &str,
    ) -> Result<OAuthCompleteResult, CoreError> {
        let tokens = self.exchange_code(provider_name, code, state).await?;

        // Fetch user info based on provider
        let user_info = match provider_name {
            "twitch" => {
                let user = self.fetch_twitch_user(&tokens.access_token).await?;
                OAuthUserInfo {
                    provider: "twitch".to_string(),
                    user_id: user.id,
                    username: user.login,
                    display_name: user.display_name,
                }
            }
            "youtube" => {
                let channel = self.fetch_youtube_channel(&tokens.access_token).await?;
                OAuthUserInfo {
                    provider: "youtube".to_string(),
                    user_id: channel.id.clone(),
                    username: channel.id, // YouTube uses channel ID as identifier
                    display_name: channel.title,
                }
            }
            _ => return Err(unknown_provider(provider_name)),
        };

        info!(
            "OAuth flow complete for {} user: {}",
            provider_name, user_info.display_name
        );

        Ok(OAuthCompleteResult { tokens, user_info })
    }

    /// Validate a Twitch token and get user info (alias for fetch_twitch_user)
    pub async fn validate_twitch_token(&self, access_token: &str) -> Result<TwitchUser, CoreError> {
        self.fetch_twitch_user(access_token).await
    }

    /// Revoke a token (best effort - not all providers support this)
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
            _ => Err(unknown_provider(provider_name)),
        }
    }
}

// ============================================================================
// OAuth Callback Server
// ============================================================================

/// Callback server for handling OAuth redirects
pub struct OAuthCallbackServer {
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl OAuthCallbackServer {
    /// Start a callback server on the specified port
    pub async fn start(port: u16) -> Result<(Self, oneshot::Receiver<OAuthCallback>), CoreError> {
        let (callback_tx, callback_rx) = oneshot::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let addr = format!("127.0.0.1:{}", port);

        // Spawn the callback server
        tokio::spawn(async move {
            Self::run_server(&addr, callback_tx, shutdown_rx).await;
        });

        Ok((
            Self {
                shutdown_tx: Some(shutdown_tx),
            },
            callback_rx,
        ))
    }

    async fn run_server(
        addr: &str,
        callback_tx: oneshot::Sender<OAuthCallback>,
        mut shutdown_rx: oneshot::Receiver<()>,
    ) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = match TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                error!("Failed to bind OAuth callback server: {}", e);
                return;
            }
        };

        info!("OAuth callback server listening on {}", addr);

        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    info!("OAuth callback server shutting down");
                    break;
                }
                result = listener.accept() => {
                    match result {
                        Ok((mut socket, _)) => {
                            let mut buffer = vec![0u8; 4096];
                            if let Ok(n) = socket.read(&mut buffer).await {
                                let request = String::from_utf8_lossy(&buffer[..n]);

                                // Parse the callback
                                if let Some(callback) = Self::parse_callback(&request) {
                                    // Send success response
                                    let response = Self::success_response();
                                    let _ = socket.write_all(response.as_bytes()).await;

                                    // Send callback to waiting handler
                                    let _ = callback_tx.send(callback);
                                    break;
                                } else {
                                    // No query params -- might be implicit flow with token in fragment.
                                    // Check if the path is /oauth/callback (without query)
                                    let first_line = request.lines().next().unwrap_or("");
                                    let path = first_line.split_whitespace().nth(1).unwrap_or("");
                                    if path.starts_with("/oauth/callback") {
                                        // Serve HTML that extracts fragment and redirects with query params
                                        let response = Self::fragment_extraction_response();
                                        let _ = socket.write_all(response.as_bytes()).await;
                                        // Don't break -- wait for the second request with query params
                                    } else {
                                        let response = Self::error_response("Invalid callback");
                                        let _ = socket.write_all(response.as_bytes()).await;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to accept connection: {}", e);
                        }
                    }
                }
            }
        }
    }

    fn parse_callback(request: &str) -> Option<OAuthCallback> {
        // Parse HTTP request to extract query params
        let first_line = request.lines().next()?;
        let path = first_line.split_whitespace().nth(1)?;

        if !path.starts_with("/oauth/callback") {
            return None;
        }

        let query = match path.split('?').nth(1) {
            Some(q) => q,
            None => {
                // No query params -- this is an implicit flow redirect where the
                // token is in the URL fragment (client-side only). Return None so
                // the callback server serves the fragment-extraction HTML page.
                return None;
            }
        };

        let params: HashMap<&str, &str> = query
            .split('&')
            .filter_map(|pair| {
                let mut parts = pair.split('=');
                Some((parts.next()?, parts.next()?))
            })
            .collect();

        // Check for error
        if let Some(error) = params.get("error") {
            return Some(OAuthCallback::Error {
                error: urlencoding::decode(error).unwrap_or_default().to_string(),
                description: params
                    .get("error_description")
                    .map(|d| urlencoding::decode(d).unwrap_or_default().to_string()),
            });
        }

        // Check for implicit flow (access_token in query params, forwarded from fragment)
        if let Some(access_token) = params.get("access_token") {
            let state = params.get("state")?;
            return Some(OAuthCallback::ImplicitSuccess {
                access_token: urlencoding::decode(access_token)
                    .unwrap_or_default()
                    .to_string(),
                state: urlencoding::decode(state).unwrap_or_default().to_string(),
            });
        }

        // Authorization code flow
        let code = params.get("code")?;
        let state = params.get("state")?;

        Some(OAuthCallback::Success {
            code: urlencoding::decode(code).unwrap_or_default().to_string(),
            state: urlencoding::decode(state).unwrap_or_default().to_string(),
        })
    }

    /// HTML page that extracts OAuth token from URL fragment (implicit flow)
    /// and redirects to the same URL with the fragment as query parameters.
    fn fragment_extraction_response() -> String {
        let body = r#"<!DOCTYPE html>
<html>
<head>
    <title>Processing Authentication...</title>
    <style>
        body { font-family: system-ui; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; background: #1a1a2e; color: #eee; }
        .container { text-align: center; }
        h1 { color: #a78bfa; }
        p { color: #9ca3af; }
    </style>
</head>
<body>
    <div class="container">
        <h1>Processing...</h1>
        <p>Completing authentication, please wait.</p>
    </div>
    <script>
        // The OAuth token is in the URL fragment (#access_token=...)
        // Fragments aren't sent to the server, so we redirect with them as query params
        if (window.location.hash) {
            var params = window.location.hash.substring(1);
            window.location.replace('/oauth/callback?' + params);
        } else {
            document.querySelector('h1').textContent = 'Authentication Failed';
            document.querySelector('p').textContent = 'No authentication data received.';
        }
    </script>
</body>
</html>"#;

        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    }

    fn success_response() -> String {
        let body = r#"<!DOCTYPE html>
<html>
<head>
    <title>Authentication Successful</title>
    <style>
        body { font-family: system-ui; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; background: #1a1a2e; color: #eee; }
        .container { text-align: center; }
        h1 { color: #a78bfa; }
        p { color: #9ca3af; }
    </style>
</head>
<body>
    <div class="container">
        <h1>Authentication Successful</h1>
        <p>You can close this window and return to SpiritStream.</p>
        <script>setTimeout(() => window.close(), 3000);</script>
    </div>
</body>
</html>"#;

        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    }

    fn error_response(message: &str) -> String {
        let body = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <title>Authentication Failed</title>
    <style>
        body {{ font-family: system-ui; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; background: #1a1a2e; color: #eee; }}
        .container {{ text-align: center; }}
        h1 {{ color: #ef4444; }}
        p {{ color: #9ca3af; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>Authentication Failed</h1>
        <p>{}</p>
    </div>
</body>
</html>"#,
            message
        );

        format!(
            "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    }

    /// Shutdown the callback server
    pub fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

/// OAuth callback result
#[derive(Debug, Clone)]
pub enum OAuthCallback {
    /// Authorization code flow callback (YouTube, etc.)
    Success { code: String, state: String },
    /// Implicit flow callback — token arrives directly in the
    /// redirect URL (Twitch implicit grant; fragment hoisted into the
    /// query by the callback HTML before SpiritStream sees it).
    ImplicitSuccess { access_token: String, state: String },
    Error {
        error: String,
        description: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svc() -> OAuthService {
        OAuthService::new(OAuthConfig::default())
    }

    /// `expires_at == 0` means "no expiry recorded" — must NOT trigger
    /// refresh, otherwise freshly-stored profiles would churn refresh
    /// requests on every API call.
    #[test]
    fn zero_expires_at_never_needs_refresh() {
        let s = svc();
        assert!(!s.token_needs_refresh(0, 300));
    }

    /// A token expiring inside the leeway window must trigger refresh; one
    /// outside the window must not.
    #[test]
    fn token_inside_leeway_window_needs_refresh() {
        let s = svc();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        assert!(
            s.token_needs_refresh(now + 60, 300),
            "60s away within 300s leeway"
        );
        assert!(
            !s.token_needs_refresh(now + 600, 300),
            "600s away outside 300s leeway"
        );
    }

    /// `refresh_if_expiring` returns `Unauthorized` when an expiring token
    /// has no refresh_token to spend — transports map this to 401 so the UI
    /// re-prompts login instead of silently retrying.
    #[tokio::test]
    async fn refresh_if_expiring_without_refresh_token_returns_unauthorized() {
        let s = svc();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let result = s.refresh_if_expiring("twitch", "", now + 60, 300).await;
        match result {
            Err(crate::errors::CoreError::Unauthorized) => {}
            other => panic!("expected Unauthorized, got {other:?}"),
        }
    }

    /// `refresh_if_expiring` is a no-op when the token is still valid.
    #[tokio::test]
    async fn refresh_if_expiring_skips_when_token_still_valid() {
        let s = svc();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        // 1 hour out, 5 minute leeway → don't refresh.
        let result = s
            .refresh_if_expiring("twitch", "any", now + 3600, 300)
            .await;
        assert!(matches!(result, Ok(None)));
    }
}
