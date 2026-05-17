use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        ConnectInfo, DefaultBodyLimit, Json, Query, State,
    },
    http::{header, HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use governor::{
    clock::DefaultClock, state::keyed::DefaultKeyedStateStore, Quota, RateLimiter,
};

/// Per-endpoint rate limiter keyed by auth-subject (or peer IP
/// when no auth is available). One per high-risk route plus a `default_auth`
/// catch-all for everything else.
type KeyedLimiter = RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>;
use chrono::{DateTime, Duration, Local, Timelike};
use log::{Level, LevelFilter, Log, Metadata, Record};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    env,
    fs::OpenOptions,
    io::Write,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    num::NonZeroU32,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};
use subtle::ConstantTimeEq;
use tokio::sync::{broadcast, Mutex as AsyncMutex};
use tower_cookies::{Cookie, CookieManagerLayer, Cookies};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
};

use spiritstream_core::models::{
    ChatConfig, ChatCredentials, ChatPlatform, ChatSettings, FileBrowseResponse, FileEntry,
    FileHomeResponse, Profile, ProfileSettings, TwitchAuth, YouTubeAuth,
};
use spiritstream_core::services::{
    prune_logs, validate_path_within_any, AuditLogService, AuthService,
    ChatManager, ConfirmTokenService, DiscordWebhookService, EventSink, FFmpegHandler,
    FFmpegLocator, OAuthService, ObsWebSocketHandler, ProfileManager, SafetyService,
    SettingsManager, ThemeManager, CONFIRM_TOKEN_TTL_SECS,
};

// Versioned REST API surface (`/api/v1/*`). New typed handlers live here.
// See `crates/transport-http/src/v1.rs` and the rewrite plan.
pub mod v1;

mod error;
pub use error::ApiError;

mod transport;
pub use transport::HttpTransport;

// ============================================================================
// Constants
// ============================================================================

const AUTH_COOKIE_NAME: &str = "spiritstream_session";
const COOKIE_MAX_AGE_SECS: i64 = 7 * 24 * 60 * 60; // 7 days
const DEFAULT_RATE_LIMIT_PER_MINUTE: u32 = 300;

// ============================================================================
// Event System
// ============================================================================

#[derive(Clone, Serialize)]
struct ServerEvent {
    event: String,
    payload: Value,
}

#[derive(Clone)]
struct EventBus {
    sender: broadcast::Sender<ServerEvent>,
}

impl EventBus {
    fn new() -> Self {
        let (sender, _) = broadcast::channel(256);
        Self { sender }
    }

    fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.sender.subscribe()
    }
}

impl EventSink for EventBus {
    fn emit(&self, event: &str, payload: Value) {
        let _ = self.sender.send(ServerEvent {
            event: event.to_string(),
            payload,
        });
    }
}

// ============================================================================
// Application State
// ============================================================================

/// Session-cookie hardening mode.
///
/// Selected at startup from `SPIRITSTREAM_COOKIE_MODE` (or auto-detected from
/// bind address and `SPIRITSTREAM_DEPLOY_MODE`). Controls the `Secure` and
/// `SameSite` attributes on the auth cookie:
///
/// * `SameOrigin` — Tauri 2 webview or Docker single-tenant. The UI and API
///   share an origin; strictest cookie policy applies.
///   `Secure; HttpOnly; SameSite=Strict; Path=/; Max-Age=604800`.
/// * `CrossOrigin` — browser UI served from a different host than the API
///   (e.g. cloud deploy with separate UI domain). `Strict` would drop the
///   cookie on cross-site navigation back to the UI, so `Lax` is used.
///   `Secure; HttpOnly; SameSite=Lax; Path=/; Max-Age=604800`.
/// * `LocalhostDev` — loopback HTTP development. `Secure` is dropped because
///   the browser refuses Secure cookies over plain HTTP, but `HttpOnly` and
///   `SameSite=Strict` still apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionCookieMode {
    SameOrigin,
    CrossOrigin,
    LocalhostDev,
}

impl SessionCookieMode {
    /// Pure decision function — given an optional `SPIRITSTREAM_COOKIE_MODE`
    /// override, the bind host, and the deploy mode, return the cookie
    /// mode. Production callers read the env var once at startup and
    /// pass it in; tests pass it directly to avoid env-mutation races.
    pub(crate) fn detect(host: &str, deploy_mode: Option<&str>, explicit: Option<&str>) -> Self {
        if let Some(explicit) = explicit {
            match explicit.to_ascii_lowercase().as_str() {
                "same_origin" | "same-origin" | "sameorigin" => return Self::SameOrigin,
                "cross_origin" | "cross-origin" | "crossorigin" => return Self::CrossOrigin,
                "localhost_dev" | "localhost-dev" | "localhostdev" | "dev" => {
                    return Self::LocalhostDev
                }
                _ => {} // unrecognised — fall through to auto-detect
            }
        }
        if matches!(deploy_mode, Some(m) if m.eq_ignore_ascii_case("cloud")) {
            return Self::CrossOrigin;
        }
        if is_loopback_host(host) {
            return Self::LocalhostDev;
        }
        Self::SameOrigin
    }

    fn secure(self) -> bool {
        !matches!(self, Self::LocalhostDev)
    }

    fn same_site(self) -> tower_cookies::cookie::SameSite {
        match self {
            Self::CrossOrigin => tower_cookies::cookie::SameSite::Lax,
            _ => tower_cookies::cookie::SameSite::Strict,
        }
    }
}

fn is_loopback_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "::1" | "localhost" | "0.0.0.0") || host.starts_with("127.")
}

/// Per-endpoint rate limit configuration.
///
/// Each high-risk endpoint has its own keyed rate limiter. The `default`
/// catch-all replaces the prior single global governor quota. Burst sizes
/// follow the plan's recommended initial values; quotas can be tuned with
/// telemetry once observability lands.
pub(crate) struct EndpointRateLimiters {
    /// `POST /api/v1/auth/login` — 5/min keyed on peer IP. Per-account
    /// exponential backoff lives in `AuthService`.
    pub login: KeyedLimiter,
    /// `POST /api/v1/chat/messages` — 20/min sustained, burst 5,
    /// keyed on auth subject.
    pub chat_send: KeyedLimiter,
    /// `POST /api/v1/streams` — 10/min sustained, burst 2, keyed on
    /// auth subject. FFmpeg spawn is expensive.
    pub stream_start: KeyedLimiter,
    /// `POST /api/v1/oauth/*/flow` — 5/min keyed on auth subject.
    pub oauth_flow: KeyedLimiter,
    /// Catch-all for every other authenticated request. Replaces the
    /// prior single global `NotKeyed` quota.
    pub default_auth: KeyedLimiter,
}

impl EndpointRateLimiters {
    fn from_env() -> Self {
        let default_quota = env::var("SPIRITSTREAM_RATE_LIMIT")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .and_then(NonZeroU32::new)
            .unwrap_or_else(|| NonZeroU32::new(DEFAULT_RATE_LIMIT_PER_MINUTE).unwrap());

        Self {
            login: RateLimiter::keyed(Quota::per_minute(NonZeroU32::new(5).unwrap())),
            chat_send: RateLimiter::keyed(
                Quota::per_minute(NonZeroU32::new(20).unwrap())
                    .allow_burst(NonZeroU32::new(5).unwrap()),
            ),
            stream_start: RateLimiter::keyed(
                Quota::per_minute(NonZeroU32::new(10).unwrap())
                    .allow_burst(NonZeroU32::new(2).unwrap()),
            ),
            oauth_flow: RateLimiter::keyed(Quota::per_minute(NonZeroU32::new(5).unwrap())),
            default_auth: RateLimiter::keyed(Quota::per_minute(default_quota)),
        }
    }
}

/// Snapshot of the active profile's PII blocklist plus the
/// fuzzy-matching flag. Shared between the activation handler and the chat
/// send path; refreshed atomically when a new profile becomes active.
pub(crate) type ActiveProfilePii = Arc<AsyncMutex<Option<(Vec<String>, bool)>>>;

/// Static HTML served at `GET /` while `ServerReadiness::ready == false`.
/// Uses `<meta http-equiv="refresh" content="1">` so the browser polls
/// without running any JavaScript; once services initialize, the next
/// refresh serves the real SPA. Same dark background as the Tauri shell
/// so users see no flash. No Content-Security-Policy issues (no inline
/// scripts, no external resources — pure HTML + minimal inline CSS).
const LOADING_PAGE_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <meta http-equiv="refresh" content="1" />
  <title>SpiritStream — Starting…</title>
  <style>
    html, body {
      margin: 0;
      padding: 0;
      height: 100%;
      background-color: #0F0A14;
      color: #F4F2F7;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    }
    .center {
      position: absolute;
      inset: 0;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      gap: 1rem;
    }
    .spinner {
      width: 28px;
      height: 28px;
      border-radius: 50%;
      border: 3px solid rgba(167, 139, 250, 0.25);
      border-top-color: #A78BFA;
      animation: spin 0.8s linear infinite;
    }
    @keyframes spin { to { transform: rotate(360deg); } }
    .label { color: #B8AECA; font-size: 0.95rem; }
  </style>
</head>
<body>
  <div class="center">
    <div class="spinner" aria-hidden="true"></div>
    <div class="label">Starting SpiritStream…</div>
  </div>
</body>
</html>
"#;

/// Tracks whether all subsystems have completed startup initialization.
/// Single source of readiness truth across every deployment shape (Tauri
/// desktop, Tauri mobile, Docker, browser). The `/api/v1/ready` long-poll
/// awaits this Notify; the SPA loading-page handler reads the AtomicBool.
/// Flipped exactly once after `AppState` is fully wired in `build_state`.
pub struct ServerReadiness {
    pub ready: std::sync::atomic::AtomicBool,
    pub notify: tokio::sync::Notify,
}

impl Default for ServerReadiness {
    fn default() -> Self {
        Self {
            ready: std::sync::atomic::AtomicBool::new(false),
            notify: tokio::sync::Notify::new(),
        }
    }
}

// Public so the `v1` REST module (and future transport adapters) can hold a
// typed handle to the wired-up service registry.
#[derive(Clone)]
pub struct AppState {
    pub(crate) profile_manager: Arc<ProfileManager>,
    pub(crate) settings_manager: Arc<SettingsManager>,
    pub(crate) ffmpeg_handler: Arc<FFmpegHandler>,
    pub(crate) ffmpeg_locator: Arc<FFmpegLocator>,
    pub(crate) theme_manager: Arc<ThemeManager>,
    pub(crate) obs_handler: Arc<ObsWebSocketHandler>,
    pub(crate) discord_service: Arc<DiscordWebhookService>,
    pub(crate) chat_manager: Arc<ChatManager>,
    pub(crate) oauth_service: Arc<OAuthService>,
    pub(crate) event_bus: EventBus,
    pub(crate) log_dir: PathBuf,
    pub(crate) app_data_dir: PathBuf,
    pub(crate) auth_token: Option<String>,
    pub(crate) active_profile_name: Arc<AsyncMutex<Option<String>>>,
    pub(crate) active_profile_settings: Arc<AsyncMutex<Option<ProfileSettings>>>,
    /// Snapshot of the active profile's PII blocklist +
    /// fuzzy flag. Updated on activate; consulted by chat send.
    pub(crate) active_profile_pii: ActiveProfilePii,
    /// Server-side per-session unlock state for encrypted profiles. Replaces
    /// the frontend `unlockedProfiles: Set<string>` that used to live in
    /// `Profiles.tsx:40-67` — moved server-side so refresh / cross-device /
    /// reload all stay consistent. Keys are profile names.
    pub(crate) unlocked_profiles: Arc<AsyncMutex<std::collections::HashSet<String>>>,
    // Allowed export directories for path validation
    pub(crate) home_dir: Option<PathBuf>,
    /// Selected once at startup, drives `set_session_cookie`.
    pub(crate) cookie_mode: SessionCookieMode,
    /// Origin allow-list used by the CSRF middleware when
    /// `Sec-Fetch-Site` is missing or reports `cross-site` (e.g. Tauri
    /// webview). Same source as the CORS allow-list (`SPIRITSTREAM_CORS_ORIGINS`).
    pub(crate) allowed_origins: Arc<Vec<String>>,
    /// Exponential backoff + sliding-window lockout for
    /// `POST /api/v1/auth/login`. Replaces the prior fixed 100ms sleep
    /// which was bypassable by pipelining.
    pub(crate) auth_service: Arc<AuthService>,
    /// Per-endpoint rate limits keyed on auth subject
    /// (fallback: peer IP). Replaces the single global governor quota.
    pub(crate) endpoint_limiters: Arc<EndpointRateLimiters>,
    /// One-shot scoped tokens that gate destructive ops
    /// (`clear_data`, `rotate_machine_key`, `revoke_all_sessions`).
    pub(crate) confirm_tokens: Arc<ConfirmTokenService>,
    /// Server-tracked active session IDs. Cookies whose
    /// value is not present here fail auth, so clearing this set
    /// invalidates every existing session immediately. Memory-only;
    /// a process restart implicitly revokes everything.
    pub(crate) active_sessions: Arc<Mutex<std::collections::HashSet<String>>>,
    /// Append-only audit log (panic, PII, OAuth refresh, etc.).
    pub(crate) audit: Arc<AuditLogService>,
    /// Coordinates the panic-disconnect flow.
    pub(crate) safety: Arc<SafetyService>,
    /// Profile-activation orchestrator. Composes profile load, OAuth
    /// refresh, chat propagation, and OBS reconfigure into a single verb
    /// so `v1_profile_activate` is a one-liner. Surveillance is plumbed
    /// into this service at construction; the HTTP transport no longer
    /// touches `AuthSurveillanceService` directly.
    pub(crate) profile_activation: Arc<spiritstream_core::services::ProfileActivationService>,
    /// Long-poll readiness — clients await this Notify on `/api/v1/ready`.
    /// `ready` flips to true exactly once after all services finish init;
    /// `notify_waiters()` wakes every parked handler at that moment.
    pub(crate) readiness: Arc<ServerReadiness>,
}

#[derive(Serialize)]
struct InvokeResponse {
    ok: bool,
    data: Option<Value>,
    error: Option<String>,
}

// ============================================================================
// Logging
// ============================================================================

struct ServerLogger {
    file: Mutex<std::fs::File>,
    event_bus: EventBus,
    level: LevelFilter,
}

impl ServerLogger {
    fn new(
        log_dir: &std::path::Path,
        event_bus: EventBus,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let log_path = log_dir.join("spiritstream-server.log");
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;
        Ok(Self {
            file: Mutex::new(file),
            event_bus,
            level: LevelFilter::Info,
        })
    }
}

impl Log for ServerLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let timestamp = Local::now();
        let target = record.target();
        let level = record.level();
        let raw_message = format!("{}", record.args());
        // Every log line passes through `mask_sensitive`
        // before it hits disk so token-shaped strings, RTMP keys, and
        // ${TOKEN} expansions can't leak via the log file. Enforced at
        // the boundary, not per call site.
        let message = mask_sensitive(&raw_message);

        // Structured JSON output when SPIRITSTREAM_LOG_FORMAT=json.
        // Default stays bracketed-text for human-readable tail-following
        // during development. Both formats apply redaction identically.
        let json_format = std::env::var("SPIRITSTREAM_LOG_FORMAT")
            .map(|v| v.eq_ignore_ascii_case("json"))
            .unwrap_or(false);
        let line = if json_format {
            // Hand-roll JSON so we don't take a serde dep for an
            // already-load-bearing fast path. Field order is stable
            // for downstream log shippers.
            format!(
                r#"{{"ts":"{}","level":"{}","target":"{}","msg":{}}}"#,
                timestamp.to_rfc3339(),
                level,
                target.replace('"', "\\\""),
                serde_json::to_string(&message).unwrap_or_else(|_| "\"\"".into()),
            )
        } else {
            let date = timestamp.format("%Y-%m-%d");
            let time = timestamp.format("%H:%M:%S");
            format!("[{date}][{time}][{target}][{level}] {message}")
        };

        if let Ok(mut file) = self.file.try_lock() {
            if let Err(e) = writeln!(file, "{line}") {
                eprintln!("Failed to write log: {e}");
            }
            // Flush after every write to ensure logs persist on crash
            if let Err(e) = file.flush() {
                eprintln!("Failed to flush log: {e}");
            }
        }

        let level_number = match level {
            Level::Error => 1,
            Level::Warn => 2,
            Level::Info => 3,
            Level::Debug => 4,
            Level::Trace => 5,
        };

        self.event_bus.emit(
            "log://log",
            json!({ "level": level_number, "message": message, "target": target }),
        );
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.try_lock() {
            let _ = file.flush();
        }
    }
}

// ============================================================================
// Security Utilities
// ============================================================================

/// Verify the `X-Confirm-Token` header against the [`ConfirmTokenService`]
/// for the given `intent`, consuming the token on success.
/// Returns an `ApiError(CoreError::PathOutsideAllowedRoot)` shaped 403
/// when missing or invalid — the same flavour of generic forbidden the
/// CSRF layer uses, so frontend error handlers don't need a new branch.
pub(crate) fn require_confirm_token(
    state: &AppState,
    headers: &HeaderMap,
    intent: &str,
) -> Result<(), ApiError> {
    let token = headers
        .get("X-Confirm-Token")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            ApiError(spiritstream_core::CoreError::PathOutsideAllowedRoot {
                path: format!("missing X-Confirm-Token for {intent}"),
            })
        })?;
    if !state.confirm_tokens.consume(intent, token) {
        return Err(ApiError(
            spiritstream_core::CoreError::PathOutsideAllowedRoot {
                path: format!("invalid or expired X-Confirm-Token for {intent}"),
            },
        ));
    }
    Ok(())
}

/// Issue a confirmation token. The frontend / CLI calls this immediately
/// before showing the user a "are you sure?" prompt, then includes the
/// returned token in the destructive request's `X-Confirm-Token` header.
#[derive(Deserialize)]
pub(crate) struct ConfirmTokenRequest {
    intent: String,
}

#[derive(Serialize)]
pub(crate) struct ConfirmTokenResponse {
    token: String,
    /// Seconds the token remains valid. Mirrors `CONFIRM_TOKEN_TTL_SECS`.
    #[serde(rename = "expiresInSeconds")]
    expires_in_seconds: u64,
}

pub(crate) async fn confirm_token_issue(
    State(state): State<AppState>,
    Json(payload): Json<ConfirmTokenRequest>,
) -> Result<Json<ConfirmTokenResponse>, ApiError> {
    let token = state.confirm_tokens.issue(&payload.intent);
    Ok(Json(ConfirmTokenResponse {
        token,
        expires_in_seconds: CONFIRM_TOKEN_TTL_SECS,
    }))
}

/// Constant-time API-token comparison.
///
/// `subtle::ConstantTimeEq` on raw `&[u8]` returns false **early** when
/// lengths differ — leaking the token length via timing. The leak is
/// small (the expected length is also bounded by env-var size), but a
/// motivated attacker could probe it with millions of requests. We
/// neutralise it by SHA-256-hashing both sides to a fixed 32-byte width
/// before the constant-time compare. This costs a single SHA-256 per
/// auth attempt — negligible compared to network RTT.
fn verify_token(expected: &str, provided: &str) -> bool {
    use sha2::{Digest, Sha256};
    let mut expected_h = Sha256::new();
    expected_h.update(expected.as_bytes());
    let mut provided_h = Sha256::new();
    provided_h.update(provided.as_bytes());
    expected_h
        .finalize()
        .as_slice()
        .ct_eq(provided_h.finalize().as_slice())
        .into()
}

/// Extract bearer token from Authorization header
fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

/// Mask sensitive patterns in arbitrary text before logging.
///
/// Four pattern families are recognised:
/// 1. Token-shaped values that follow a keyword (`token=…`, `Authorization: …`,
///    `password: …`, etc.) — the trailing value is replaced with `[REDACTED]`.
/// 2. RTMP URLs of the form `rtmp[s]://host[:port]/app/STREAM_KEY` — the
///    trailing path segment (the stream key) is masked. Twitch and YouTube
///    embed the secret key as the last URL segment, so any RTMP URL in
///    logs that survives without masking leaks the broadcast key.
/// 3. FFmpeg-style `${TOKEN}` template expansions — the contents inside
///    the braces are replaced with `[REDACTED]` so command-line dumps
///    of templated args don't ship the secret.
/// 4. `ENC::` (V1) and `ENC2::` (V2) ciphertext blobs — replaced with
///    `[ENCRYPTED]` so encrypted-at-rest values don't appear verbatim.
fn mask_sensitive(text: &str) -> String {
    use regex::Regex;
    use std::sync::OnceLock;

    static TOKEN_RE: OnceLock<Regex> = OnceLock::new();
    static ENC_RE: OnceLock<Regex> = OnceLock::new();
    static RTMP_RE: OnceLock<Regex> = OnceLock::new();
    static TEMPLATE_RE: OnceLock<Regex> = OnceLock::new();

    let token_re = TOKEN_RE.get_or_init(|| {
        Regex::new(r#"(?i)(token|key|password|secret|bearer|oauth|access_token|refresh_token|authorization)[=:\s]+['"]?([A-Za-z0-9_\-./+]{20,})['"]?"#).unwrap()
    });
    let enc_re = ENC_RE.get_or_init(|| Regex::new(r#"ENC2?::[A-Za-z0-9+/=]{10,}"#).unwrap());
    let rtmp_re = RTMP_RE.get_or_init(|| {
        // rtmp[s]://host[:port]/app/STREAM_KEY[?query]
        // Captures up to and including the application path, then masks
        // the trailing stream-key segment. Allows query strings to
        // survive (useful for debugging) but redacts the key.
        Regex::new(r"(?i)(rtmps?://[^\s/]+(?:/[^\s/?#]+){1,2}/)([^\s/?#]+)").unwrap()
    });
    let template_re = TEMPLATE_RE.get_or_init(|| Regex::new(r"\$\{([^}]+)\}").unwrap());

    let result = token_re.replace_all(text, "$1=[REDACTED]");
    let result = enc_re.replace_all(&result, "[ENCRYPTED]");
    let result = rtmp_re.replace_all(&result, "$1[REDACTED]");
    template_re
        .replace_all(&result, "${[REDACTED]}")
        .to_string()
}

/// Redact sensitive keys from a JSON payload before logging
pub(crate) fn redact_payload(value: &Value) -> Value {
    const REDACT_KEYS: &[&str] = &[
        "token",
        "key",
        "password",
        "secret",
        "oauth",
        "accessToken",
        "refreshToken",
        "oauthToken",
        "apiKey",
        "access_token",
        "refresh_token",
        "session_token",
        "webhookUrl",
    ];

    match value {
        Value::Object(map) => {
            let mut redacted = serde_json::Map::new();
            for (k, v) in map {
                let lower = k.to_lowercase();
                if REDACT_KEYS
                    .iter()
                    .any(|s| lower.contains(&s.to_lowercase()))
                {
                    if let Value::String(s) = v {
                        if !s.is_empty() {
                            redacted.insert(k.clone(), Value::String("[REDACTED]".to_string()));
                        } else {
                            redacted.insert(k.clone(), v.clone());
                        }
                    } else {
                        redacted.insert(k.clone(), Value::String("[REDACTED]".to_string()));
                    }
                } else {
                    redacted.insert(k.clone(), redact_payload(v));
                }
            }
            Value::Object(redacted)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(redact_payload).collect()),
        other => other.clone(),
    }
}

// ============================================================================
// Chat Auto-Connect / Auto-Disconnect (tied to stream lifecycle)
// ============================================================================

pub(crate) struct FreshOAuthToken {
    pub(crate) access_token: String,
    pub(crate) refresh_token: Option<String>,
    pub(crate) expires_at: i64,
    pub(crate) refreshed: bool,
}

/// Ensure an OAuth token is fresh, refreshing it via the OAuth service if expired.
/// Returns token details and whether a refresh occurred.
/// Adds a 5-minute buffer so we refresh tokens that will expire within the next 5 minutes.
///
/// Failure modes return structured `CoreError` variants:
/// - Missing access token / refresh token → `Unauthorized` (caller prompts re-login)
/// - Provider refresh call failure → propagated from `OAuthService::refresh_token`
///   (either `Unauthorized` for 4xx or `NetworkError` for 5xx / transport errors)
pub(crate) async fn ensure_fresh_oauth_token(
    provider: &str,
    access_token: &str,
    refresh_token: &str,
    expires_at: i64,
    oauth_service: &OAuthService,
) -> Result<FreshOAuthToken, spiritstream_core::CoreError> {
    if access_token.is_empty() {
        log::warn!("No {provider} OAuth token available");
        return Err(spiritstream_core::CoreError::Unauthorized);
    }

    // Check if token is expired or will expire within 5 minutes.
    let now = chrono::Utc::now().timestamp();
    let needs_refresh = expires_at > 0 && now >= (expires_at - 300);

    if !needs_refresh {
        return Ok(FreshOAuthToken {
            access_token: access_token.to_string(),
            refresh_token: None,
            expires_at,
            refreshed: false,
        });
    }

    if refresh_token.is_empty() {
        log::warn!("{provider} token expired and no refresh token available");
        return Err(spiritstream_core::CoreError::Unauthorized);
    }

    log::info!(
        "{} OAuth token expired (expired {}s ago), refreshing...",
        provider,
        now - expires_at
    );

    let tokens = oauth_service.refresh_token(provider, refresh_token).await?;
    let new_expires_at = tokens.expires_in.map(|s| now + s as i64).unwrap_or(0);
    log::info!(
        "{} OAuth token refreshed successfully (new expiry in {}s)",
        provider,
        tokens.expires_in.unwrap_or(0)
    );

    Ok(FreshOAuthToken {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        expires_at: new_expires_at,
        refreshed: true,
    })
}

pub(crate) fn build_hour_keys(start: DateTime<Local>, end: DateTime<Local>) -> Vec<String> {
    let mut keys = Vec::new();

    let start_hour = start
        .with_minute(0)
        .and_then(|d| d.with_second(0))
        .and_then(|d| d.with_nanosecond(0))
        .unwrap_or(start);
    let end_hour = end
        .with_minute(0)
        .and_then(|d| d.with_second(0))
        .and_then(|d| d.with_nanosecond(0))
        .unwrap_or(end);

    let mut cursor = start_hour;
    while cursor <= end_hour {
        keys.push(cursor.format("%Y%m%d-%H").to_string());
        cursor += Duration::hours(1);
    }

    keys
}

/// Persist the transport-level active-profile snapshot and re-emit the
/// Hydrate session-scoped transport state (`active_profile_name`,
/// `active_profile_settings`, `active_profile_pii`) and push the
/// anonymous-mode policy into `ChatManager` after a profile activates.
/// The `profile_activated` event is emitted by `ProfileActivationService`
/// itself before this runs, so both transports see identical bus shape
/// without re-emission here.
pub(crate) async fn set_active_profile(state: &AppState, profile: &Profile) {
    {
        let mut guard = state.active_profile_name.lock().await;
        *guard = Some(profile.name.clone());
    }
    {
        let mut guard = state.active_profile_settings.lock().await;
        *guard = Some(profile.settings.clone());
    }
    {
        let mut guard = state.active_profile_pii.lock().await;
        *guard = Some((profile.pii_blocklist.clone(), profile.pii_fuzzy));
    }
    // Push the anonymous-mode policy into ChatManager so
    // subsequent inbound chat messages get pseudonymised before they
    // reach the log writer or the event stream.
    state
        .chat_manager
        .set_anonymous_policy(profile.anonymous_logging, profile.anonymous_salt.clone())
        .await;
}

pub(crate) async fn get_active_profile_name(state: &AppState) -> Option<String> {
    let guard = state.active_profile_name.lock().await;
    guard.clone()
}

pub(crate) async fn get_active_profile_settings(state: &AppState) -> Option<ProfileSettings> {
    let guard = state.active_profile_settings.lock().await;
    guard.clone()
}

pub(crate) async fn set_active_profile_settings_only(state: &AppState, settings: ProfileSettings) {
    let mut guard = state.active_profile_settings.lock().await;
    *guard = Some(settings);
}

pub(crate) async fn persist_active_profile_settings(
    state: &AppState,
    settings: ProfileSettings,
) -> Result<(), spiritstream_core::CoreError> {
    let name = get_active_profile_name(state)
        .await
        .ok_or(spiritstream_core::CoreError::NoActiveProfile)?;

    // Update in-memory settings immediately so UI can reflect the change
    set_active_profile_settings_only(state, settings.clone()).await;

    let mut profile = state
        .profile_manager
        .load_with_key_decryption(&name, None)
        .await?;
    profile.settings = settings;

    state
        .profile_manager
        .save_with_key_encryption(&profile, None)
        .await?;

    Ok(())
}

pub(crate) async fn update_profile_oauth_account(
    state: &AppState,
    provider: &str,
    access_token: String,
    refresh_token: Option<String>,
    expires_at: i64,
    user_info: &spiritstream_core::services::OAuthUserInfo,
) -> Result<(), spiritstream_core::CoreError> {
    let mut profile_settings = get_active_profile_settings(state)
        .await
        .ok_or(spiritstream_core::CoreError::NoActiveProfile)?;

    match provider {
        "twitch" => {
            profile_settings.oauth.twitch.access_token = access_token;
            if let Some(rt) = refresh_token {
                profile_settings.oauth.twitch.refresh_token = rt;
            }
            profile_settings.oauth.twitch.expires_at = expires_at;
            profile_settings.oauth.twitch.user_id = user_info.user_id.clone();
            profile_settings.oauth.twitch.username = user_info.username.clone();
            profile_settings.oauth.twitch.display_name = user_info.display_name.clone();
        }
        "youtube" => {
            profile_settings.oauth.youtube.access_token = access_token;
            if let Some(rt) = refresh_token {
                profile_settings.oauth.youtube.refresh_token = rt;
            }
            profile_settings.oauth.youtube.expires_at = expires_at;
            profile_settings.oauth.youtube.user_id = user_info.user_id.clone();
            profile_settings.oauth.youtube.username = user_info.username.clone();
            profile_settings.oauth.youtube.display_name = user_info.display_name.clone();
        }
        _ => {
            return Err(spiritstream_core::CoreError::NotImplemented {
                feature: format!("Unknown provider: {provider}"),
            })
        }
    }

    persist_active_profile_settings(state, profile_settings).await
}

pub(crate) async fn clear_profile_oauth_account(
    state: &AppState,
    provider: &str,
) -> Result<(), spiritstream_core::CoreError> {
    let mut profile_settings = get_active_profile_settings(state)
        .await
        .ok_or(spiritstream_core::CoreError::NoActiveProfile)?;

    match provider {
        "twitch" => {
            profile_settings.oauth.twitch = Default::default();
        }
        "youtube" => {
            profile_settings.oauth.youtube = Default::default();
        }
        _ => {
            return Err(spiritstream_core::CoreError::NotImplemented {
                feature: format!("Unknown provider: {provider}"),
            })
        }
    }

    persist_active_profile_settings(state, profile_settings).await
}

/// Auto-connect all configured chat platforms when a stream starts.
/// Runs as a fire-and-forget background task -- errors are logged, never block the stream.
pub(crate) async fn auto_connect_chat_platforms(state: AppState) {
    let chat_settings = state.chat_manager.profile_chat_settings().await;
    if chat_settings.twitch_channel.trim().is_empty()
        && chat_settings.youtube_channel_id.trim().is_empty()
        && chat_settings.trovo_channel_id.trim().is_empty()
    {
        return;
    }

    let mut profile_settings = match get_active_profile_settings(&state).await {
        Some(settings) => settings,
        None => {
            log::warn!("Chat auto-connect skipped: no active profile settings");
            return;
        }
    };

    // Twitch: refresh token if needed, then connect immediately (IRC works even when offline)
    if !chat_settings.twitch_channel.is_empty() {
        let already_connected = state
            .chat_manager
            .get_platform_status(ChatPlatform::Twitch)
            .await
            .map(|s| s.status == spiritstream_core::models::ChatConnectionStatus::Connected)
            .unwrap_or(false);

        if !already_connected {
            if !profile_settings.oauth.twitch.access_token.is_empty() {
                // Refresh token if expired
                match ensure_fresh_oauth_token(
                    "twitch",
                    &profile_settings.oauth.twitch.access_token,
                    &profile_settings.oauth.twitch.refresh_token,
                    profile_settings.oauth.twitch.expires_at,
                    &state.oauth_service,
                )
                .await
                {
                    Ok(fresh) => {
                        if fresh.refreshed {
                            profile_settings.oauth.twitch.access_token = fresh.access_token.clone();
                            if let Some(rt) = fresh.refresh_token {
                                profile_settings.oauth.twitch.refresh_token = rt;
                            }
                            profile_settings.oauth.twitch.expires_at = fresh.expires_at;
                            if let Err(err) =
                                persist_active_profile_settings(&state, profile_settings.clone())
                                    .await
                            {
                                log::warn!("Failed to persist Twitch OAuth refresh: {err}");
                            }
                        }
                        connect_twitch_chat(
                            &state.chat_manager,
                            &chat_settings,
                            &profile_settings,
                            &state.event_bus,
                        )
                        .await;
                    }
                    Err(e) => {
                        log::warn!("Twitch token refresh failed, trying with existing token: {e}");
                        connect_twitch_chat(
                            &state.chat_manager,
                            &chat_settings,
                            &profile_settings,
                            &state.event_bus,
                        )
                        .await;
                    }
                }
            } else {
                connect_twitch_chat(
                    &state.chat_manager,
                    &chat_settings,
                    &profile_settings,
                    &state.event_bus,
                )
                .await;
            }
        } else {
            log::debug!("Twitch chat already connected, skipping auto-connect");
        }
    }

    // Trovo: read-only websocket chat (requires TROVO_CLIENT_ID + channel ID)
    if !chat_settings.trovo_channel_id.is_empty() {
        let already_connected = state
            .chat_manager
            .get_platform_status(ChatPlatform::Trovo)
            .await
            .map(|s| s.status == spiritstream_core::models::ChatConnectionStatus::Connected)
            .unwrap_or(false);

        if !already_connected {
            connect_trovo_chat(&state.chat_manager, &chat_settings, &state.event_bus).await;
        } else {
            log::debug!("Trovo chat already connected, skipping auto-connect");
        }
    }

    // YouTube: connect with retry (broadcast won't be live until OBS starts streaming)
    if !chat_settings.youtube_channel_id.is_empty() {
        let has_oauth = !chat_settings.youtube_use_api_key
            && !profile_settings.oauth.youtube.access_token.is_empty();
        let has_api_key =
            chat_settings.youtube_use_api_key && !chat_settings.youtube_api_key.is_empty();

        if has_oauth || has_api_key {
            let already_connected = state
                .chat_manager
                .get_platform_status(ChatPlatform::YouTube)
                .await
                .map(|s| s.status == spiritstream_core::models::ChatConnectionStatus::Connected)
                .unwrap_or(false);

            if !already_connected {
                // Spawn as separate task -- retries can take up to 5 minutes
                tokio::spawn(connect_youtube_chat_with_retry(state.clone()));
            } else {
                log::debug!("YouTube chat already connected, skipping auto-connect");
            }
        }
    }
}

pub(crate) async fn connect_twitch_chat(
    chat_manager: &Arc<ChatManager>,
    chat_settings: &ChatSettings,
    profile_settings: &ProfileSettings,
    event_bus: &EventBus,
) {
    let auth = if profile_settings.oauth.twitch.access_token.is_empty() {
        None
    } else {
        Some(TwitchAuth::AppOAuth {
            access_token: profile_settings.oauth.twitch.access_token.clone(),
            refresh_token: Some(profile_settings.oauth.twitch.refresh_token.clone())
                .filter(|s| !s.is_empty()),
            expires_at: if profile_settings.oauth.twitch.expires_at > 0 {
                Some(profile_settings.oauth.twitch.expires_at)
            } else {
                None
            },
        })
    };

    let config = ChatConfig {
        platform: ChatPlatform::Twitch,
        enabled: true,
        credentials: ChatCredentials::Twitch {
            channel: chat_settings.twitch_channel.clone(),
            auth,
        },
    };
    match chat_manager.connect(config).await {
        Ok(()) => {
            log::info!("Auto-connected to Twitch chat");
            event_bus.emit("chat_auto_connected", json!({ "platform": "twitch" }));
        }
        Err(e) => {
            if e.to_string().to_lowercase().contains("already connected") {
                log::debug!("Twitch chat already connected");
            } else {
                log::warn!("Failed to auto-connect Twitch chat: {e}");
                // Surface auto-connect failures to the UI so the
                // user gets a toast instead of silently missing chat. The
                // stream itself continues regardless.
                event_bus.emit(
                    "chat_auto_connect_failed",
                    json!({ "platform": "twitch", "kind": e.kind(), "error": e.to_string() }),
                );
            }
        }
    }
}

pub(crate) async fn connect_trovo_chat(
    chat_manager: &Arc<ChatManager>,
    chat_settings: &ChatSettings,
    event_bus: &EventBus,
) {
    let config = ChatConfig {
        platform: ChatPlatform::Trovo,
        enabled: true,
        credentials: ChatCredentials::Trovo {
            channel_id: chat_settings.trovo_channel_id.clone(),
        },
    };
    match chat_manager.connect(config).await {
        Ok(()) => {
            log::info!("Auto-connected to Trovo chat");
            event_bus.emit("chat_auto_connected", json!({ "platform": "trovo" }));
        }
        Err(e) => {
            if e.to_string().to_lowercase().contains("already connected") {
                log::debug!("Trovo chat already connected");
            } else {
                log::warn!("Failed to auto-connect Trovo chat: {e}");
                event_bus.emit(
                    "chat_auto_connect_failed",
                    json!({ "platform": "trovo", "kind": e.kind(), "error": e.to_string() }),
                );
            }
        }
    }
}

/// Wait for stream data to flow, then connect YouTube chat.
///
/// SpiritStream starts its RTMP relay *before* OBS connects, so the YouTube
/// broadcast won't be "active" until OBS is streaming and YouTube has ingested
/// enough data. We subscribe to the EventBus, wait for the first `stream_stats`
/// event (proof that data is flowing from OBS), give YouTube time to register
/// the broadcast, then attempt to connect with a few retries.
pub(crate) async fn connect_youtube_chat_with_retry(state: AppState) {
    let build_config = |chat: &ChatSettings,
                        s: &ProfileSettings,
                        token_override: Option<&str>|
     -> Option<ChatConfig> {
        if chat.youtube_channel_id.trim().is_empty() {
            return None;
        }

        let auth = if chat.youtube_use_api_key {
            if chat.youtube_api_key.trim().is_empty() {
                return None;
            }
            YouTubeAuth::ApiKey {
                key: chat.youtube_api_key.clone(),
            }
        } else {
            let access_token = token_override.unwrap_or(&s.oauth.youtube.access_token);
            if access_token.is_empty() {
                return None;
            }
            YouTubeAuth::AppOAuth {
                access_token: access_token.to_string(),
                refresh_token: Some(s.oauth.youtube.refresh_token.clone())
                    .filter(|t| !t.is_empty()),
                expires_at: if s.oauth.youtube.expires_at > 0 {
                    Some(s.oauth.youtube.expires_at)
                } else {
                    None
                },
            }
        };

        Some(ChatConfig {
            platform: ChatPlatform::YouTube,
            enabled: true,
            credentials: ChatCredentials::YouTube {
                channel_id: chat.youtube_channel_id.clone(),
                auth,
            },
        })
    };

    let initial_chat = state.chat_manager.profile_chat_settings().await;
    if initial_chat.youtube_channel_id.trim().is_empty() {
        return;
    }

    // Wait for stream_stats (OBS connected, data flowing)
    log::info!("YouTube chat: waiting for stream data before connecting...");
    let mut rx = state.event_bus.subscribe();
    let got_stats = tokio::time::timeout(
        std::time::Duration::from_secs(300), // 5 min max wait for OBS
        async {
            loop {
                match rx.recv().await {
                    Ok(event) if event.event == "stream_stats" => return,
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => return, // channel closed
                }
            }
        },
    )
    .await;

    if got_stats.is_err() {
        log::warn!("YouTube chat: timed out waiting for stream data (OBS never connected?)");
        return;
    }
    if state.ffmpeg_handler.active_count() == 0 {
        log::info!("YouTube chat: stream stopped before OBS data arrived");
        return;
    }

    // Data is flowing. Give YouTube ~10s to register the broadcast.
    log::info!("Stream data detected -- waiting 10s for YouTube to register broadcast...");
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    // Attempt connect with a few retries (15s apart)
    // Load fresh settings each attempt so we pick up refreshed tokens
    const MAX_RETRIES: u32 = 6; // 6 x 15s = 90s of retries after initial wait
    for attempt in 0..=MAX_RETRIES {
        if state.ffmpeg_handler.active_count() == 0 {
            log::info!("YouTube chat: stream stopped, cancelling connect");
            return;
        }

        let chat_settings = state.chat_manager.profile_chat_settings().await;
        if chat_settings.youtube_channel_id.trim().is_empty() {
            log::info!("YouTube chat: no channel configured, cancelling connect");
            return;
        }
        if chat_settings.youtube_use_api_key && chat_settings.youtube_api_key.trim().is_empty() {
            log::info!("YouTube chat: API key mode enabled but no key configured");
            return;
        }

        // Load fresh profile settings and refresh token if needed
        let mut profile_settings = match get_active_profile_settings(&state).await {
            Some(s) => s,
            None => {
                log::warn!("YouTube chat: no active profile settings");
                return;
            }
        };

        // Refresh YouTube OAuth token if expired (skip for API key mode)
        let fresh_token = if !chat_settings.youtube_use_api_key
            && !profile_settings.oauth.youtube.access_token.is_empty()
        {
            match ensure_fresh_oauth_token(
                "youtube",
                &profile_settings.oauth.youtube.access_token,
                &profile_settings.oauth.youtube.refresh_token,
                profile_settings.oauth.youtube.expires_at,
                &state.oauth_service,
            )
            .await
            {
                Ok(fresh) => {
                    if fresh.refreshed {
                        profile_settings.oauth.youtube.access_token = fresh.access_token.clone();
                        if let Some(rt) = fresh.refresh_token {
                            profile_settings.oauth.youtube.refresh_token = rt;
                        }
                        profile_settings.oauth.youtube.expires_at = fresh.expires_at;
                        if let Err(err) =
                            persist_active_profile_settings(&state, profile_settings.clone()).await
                        {
                            log::warn!("Failed to persist YouTube OAuth refresh: {err}");
                        }
                    }
                    Some(fresh.access_token)
                }
                Err(e) => {
                    log::warn!("YouTube token refresh failed: {e}");
                    None // try with existing token anyway
                }
            }
        } else {
            None
        };

        let config = match build_config(&chat_settings, &profile_settings, fresh_token.as_deref()) {
            Some(config) => config,
            None => {
                log::info!("YouTube chat: missing auth or channel info, cancelling connect");
                return;
            }
        };

        match state.chat_manager.connect(config).await {
            Ok(()) => {
                log::info!("Auto-connected to YouTube chat");
                state
                    .event_bus
                    .emit("chat_auto_connected", json!({ "platform": "youtube" }));
                return;
            }
            Err(e) => {
                let lower = e.to_string().to_lowercase();
                if lower.contains("already connected") {
                    log::debug!("YouTube chat already connected");
                    return;
                } else if lower.contains("no active live broadcast") || lower.contains("not live") {
                    if attempt < MAX_RETRIES {
                        log::info!(
                            "YouTube broadcast not live yet (attempt {}/{}), retrying in 15s...",
                            attempt + 1,
                            MAX_RETRIES + 1
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
                    } else {
                        log::warn!("YouTube chat: broadcast never went live after all retries");
                        state.event_bus.emit(
                            "chat_auto_connect_failed",
                            json!({ "platform": "youtube", "kind": e.kind(), "error": e.to_string() }),
                        );
                    }
                } else {
                    log::warn!("Failed to auto-connect YouTube chat: {e}");
                    state.event_bus.emit(
                        "chat_auto_connect_failed",
                        json!({ "platform": "youtube", "kind": e.kind(), "error": e.to_string() }),
                    );
                    return;
                }
            }
        }
    }
}

/// Auto-disconnect all chat platforms when all streams stop.
pub(crate) async fn auto_disconnect_chat_platforms(
    chat_manager: Arc<ChatManager>,
    event_bus: EventBus,
) {
    if chat_manager.is_any_connected().await {
        match chat_manager.disconnect_all().await {
            Ok(()) => {
                log::info!("Auto-disconnected all chat platforms");
                event_bus.emit("chat_auto_disconnected", json!({}));
            }
            Err(e) => {
                log::warn!("Failed to auto-disconnect chat: {e}");
            }
        }
    }
}

/// Background task to refresh YouTube OAuth tokens and update the live chat connector.
async fn start_youtube_token_refresh_task(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            interval.tick().await;

            let is_connected = state
                .chat_manager
                .get_platform_status(ChatPlatform::YouTube)
                .await
                .map(|s| s.status == spiritstream_core::models::ChatConnectionStatus::Connected)
                .unwrap_or(false);

            if !is_connected {
                continue;
            }

            let mut profile_settings = match get_active_profile_settings(&state).await {
                Some(s) => s,
                None => {
                    log::warn!("YouTube token refresh: no active profile settings");
                    continue;
                }
            };

            if profile_settings.oauth.youtube.access_token.is_empty()
                || profile_settings.oauth.youtube.refresh_token.is_empty()
                || profile_settings.oauth.youtube.expires_at <= 0
            {
                continue;
            }

            let previous_token = profile_settings.oauth.youtube.access_token.clone();
            match ensure_fresh_oauth_token(
                "youtube",
                &previous_token,
                &profile_settings.oauth.youtube.refresh_token,
                profile_settings.oauth.youtube.expires_at,
                &state.oauth_service,
            )
            .await
            {
                Ok(fresh) => {
                    if fresh.refreshed {
                        profile_settings.oauth.youtube.access_token = fresh.access_token.clone();
                        if let Some(rt) = fresh.refresh_token {
                            profile_settings.oauth.youtube.refresh_token = rt;
                        }
                        profile_settings.oauth.youtube.expires_at = fresh.expires_at;
                        if let Err(err) =
                            persist_active_profile_settings(&state, profile_settings.clone()).await
                        {
                            log::warn!("Failed to persist YouTube OAuth refresh: {err}");
                        }
                    }
                    if fresh.access_token != previous_token {
                        if let Err(e) = state
                            .chat_manager
                            .update_platform_token(ChatPlatform::YouTube, fresh.access_token)
                            .await
                        {
                            log::warn!("Failed to update YouTube chat token: {e}");
                        } else {
                            log::info!("YouTube chat token refreshed and updated");
                        }
                    }
                }
                Err(e) => {
                    log::warn!("YouTube token refresh failed: {e}");
                }
            }
        }
    });
}

/// Background task to retry chat connections when a platform drops.
/// Subscribe to the server's own event bus and auto-retry failed streams.
/// Replaces the frontend's `handleAutoRetry` (in `useStreamStats.ts`) which
/// previously listened to `stream_error` and called `api.stream.retry()`.
/// Backend now owns both the policy (in `FFmpegHandler::retry_group`) and
/// the trigger. The frontend just renders `stream_retry_attempt` events.
async fn start_auto_retry_task(state: AppState) {
    let mut rx = state.event_bus.subscribe();
    tokio::spawn(async move {
        loop {
            let evt = match rx.recv().await {
                Ok(e) => e,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            };
            if evt.event != "stream_error" {
                continue;
            }
            let Some(group_id) = evt
                .payload
                .get("groupId")
                .and_then(|v| v.as_str())
                .map(String::from)
            else {
                continue;
            };
            // Spawn the retry in a blocking task — retry_group_internal sleeps
            // for the backoff window which must not block the runtime.
            let handler = state.ffmpeg_handler.clone();
            let event_sink: Arc<dyn EventSink> = Arc::new(state.event_bus.clone());
            tokio::task::spawn_blocking(move || {
                if let Err(err) = handler.retry_group(&group_id, event_sink) {
                    log::warn!("[auto_retry] retry_group({group_id}) failed: {err}");
                }
            });
        }
    });
}

async fn start_chat_reconnect_task(state: AppState) {
    tokio::spawn(async move {
        let mut last_attempts: HashMap<ChatPlatform, Instant> = HashMap::new();
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            interval.tick().await;

            if state.ffmpeg_handler.active_count() == 0 {
                continue;
            }

            let statuses = state.chat_manager.get_status().await;
            for status in statuses {
                if status.status != spiritstream_core::models::ChatConnectionStatus::Error {
                    continue;
                }

                if last_attempts
                    .get(&status.platform)
                    .map(|last| last.elapsed() < std::time::Duration::from_secs(30))
                    .unwrap_or(false)
                {
                    continue;
                }
                last_attempts.insert(status.platform, Instant::now());

                let profile_settings = match get_active_profile_settings(&state).await {
                    Some(s) => s,
                    None => {
                        log::warn!("Chat reconnect: no active profile settings");
                        continue;
                    }
                };
                let chat_settings = state.chat_manager.profile_chat_settings().await;

                match status.platform {
                    ChatPlatform::Twitch => {
                        if chat_settings.twitch_channel.is_empty() {
                            continue;
                        }
                        connect_twitch_chat(
                            &state.chat_manager,
                            &chat_settings,
                            &profile_settings,
                            &state.event_bus,
                        )
                        .await;
                    }
                    ChatPlatform::Trovo => {
                        if chat_settings.trovo_channel_id.is_empty() {
                            continue;
                        }
                        connect_trovo_chat(&state.chat_manager, &chat_settings, &state.event_bus)
                            .await;
                    }
                    ChatPlatform::YouTube => {
                        let has_oauth = !chat_settings.youtube_use_api_key
                            && !profile_settings.oauth.youtube.access_token.is_empty();
                        let has_api_key = chat_settings.youtube_use_api_key
                            && !chat_settings.youtube_api_key.is_empty();
                        if chat_settings.youtube_channel_id.is_empty()
                            || (!has_oauth && !has_api_key)
                        {
                            continue;
                        }
                        tokio::spawn(connect_youtube_chat_with_retry(state.clone()));
                    }
                    _ => {}
                }
            }
        }
    });
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Refuse to start in cloud mode without the safety net.
///
/// Two preconditions are checked, both load-bearing for a public
/// deployment:
///
/// 1. **Strong API token**: `SPIRITSTREAM_API_TOKEN` (or, falling back,
///    a profile-stored backend token) must be present and at least 32
///    characters. Anything weaker is brute-force-trivial over the
///    public internet even with the lockout in place.
/// 2. **TLS in front**: the operator must set
///    `SPIRITSTREAM_BEHIND_TLS_PROXY=1` to declare that a reverse
///    proxy (Caddy / Traefik / nginx) terminates TLS in front of the
///    server. Cloud-mode HTTP-only is never acceptable — bearer
///    tokens and session cookies would flow in plaintext.
///
/// `pre_deploy_mode == "cloud"` is the only trigger; localhost dev
/// and `desktop` mode never hit this path.
fn enforce_cloud_mode_preconditions(
    auth_token: &Option<String>,
    tls_declared: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    const MIN_TOKEN_LEN: usize = 32;
    let token_ok = auth_token
        .as_deref()
        .map(|t| t.len() >= MIN_TOKEN_LEN)
        .unwrap_or(false);
    if !token_ok {
        return Err(format!(
            "SPIRITSTREAM_DEPLOY_MODE=cloud refuses to start: \
             SPIRITSTREAM_API_TOKEN must be ≥ {MIN_TOKEN_LEN} characters. \
             Generate one with `openssl rand -base64 32`."
        )
        .into());
    }
    if !tls_declared {
        return Err("SPIRITSTREAM_DEPLOY_MODE=cloud refuses to start: \
             set SPIRITSTREAM_BEHIND_TLS_PROXY=1 once a TLS-terminating \
             reverse proxy (Caddy / Traefik / nginx) is in place. \
             Cloud deployments without TLS leak session tokens in cleartext."
            .into());
    }
    log::info!("Cloud-mode preconditions satisfied: strong API token + TLS-fronted declared.");
    Ok(())
}

// ============================================================================
// CORS / CSRF Origin allow-list
// ============================================================================

/// Parse the `SPIRITSTREAM_CORS_ORIGINS` env var into a flat allow-list.
/// Shared by `build_cors_layer` and the CSRF middleware so the policy can
/// never drift between the browser-enforced CORS check and the
/// server-enforced CSRF fallback.
fn allowed_origins_from_env() -> Vec<String> {
    let cors_origins = env::var("SPIRITSTREAM_CORS_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:*,http://127.0.0.1:*,tauri://localhost,http://tauri.localhost,https://tauri.localhost".to_string());
    cors_origins
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// True if `origin` matches any pattern in `allowed`. Patterns ending in
/// `:*` wildcard the port (e.g. `http://localhost:*` matches any port).
fn origin_matches(origin: &str, allowed: &[String]) -> bool {
    allowed.iter().any(|pattern| {
        if let Some(prefix) = pattern.strip_suffix(":*") {
            origin.starts_with(prefix) && origin[prefix.len()..].starts_with(':')
        } else {
            origin == pattern
        }
    })
}

fn build_cors_layer(allowed_origins: Arc<Vec<String>>) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(move |origin: &HeaderValue, _| {
            let Ok(origin_str) = origin.to_str() else {
                return false;
            };
            origin_matches(origin_str, &allowed_origins)
        }))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::COOKIE,
            header::AUTHORIZATION,
            // Destructive ops (clear-data, rotate-machine-key,
            // revoke-all-sessions) carry a one-shot confirm token in this
            // header. Without it on the CORS allow-list, the preflight
            // OPTIONS response omits the header and the browser blocks
            // the actual POST with a generic "Load failed". Plain-fetch
            // requests don't show this because they have no custom headers
            // to preflight on.
            HeaderName::from_static("x-confirm-token"),
        ])
        .allow_credentials(true)
}

// ============================================================================
// Authentication Endpoints
// ============================================================================

#[derive(Deserialize)]
struct LoginRequest {
    token: String,
}

/// Build a hardened session cookie under the configured [`SessionCookieMode`]
/// and register the new session ID with the server-side active-sessions
/// set.
///
/// Attribute matrix:
/// * `SameOrigin`   → `Secure; HttpOnly; SameSite=Strict`
/// * `CrossOrigin`  → `Secure; HttpOnly; SameSite=Lax`
/// * `LocalhostDev` → `HttpOnly; SameSite=Strict` (no Secure over plain HTTP)
///
/// All variants set `Path=/` and `Max-Age = COOKIE_MAX_AGE_SECS` (7 days).
fn set_session_cookie(state: &AppState, cookies: &Cookies) {
    let session_id = uuid::Uuid::new_v4().to_string();
    {
        let mut sessions = state
            .active_sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        sessions.insert(session_id.clone());
    }
    let mode = state.cookie_mode;
    let cookie = Cookie::build((AUTH_COOKIE_NAME, session_id))
        .http_only(true)
        .secure(mode.secure())
        .same_site(mode.same_site())
        .path("/")
        .max_age(tower_cookies::cookie::time::Duration::seconds(
            COOKIE_MAX_AGE_SECS,
        ))
        .build();
    cookies.add(cookie);
}

// Typed REST response structs for the auth + session-management surface.
// All success responses are direct `Json<T>`; failures route through
// `ApiError(CoreError)` per the transport contract.

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct AuthLoginResponse {}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct AuthLogoutResponse {}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthCheckResponse {
    pub authenticated: bool,
    pub required: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RevokeAllSessionsResponse {
    pub revoked: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct FilesOpenResponse {}

const AUTH_ACCOUNT_DEFAULT: &str = "default";

fn lockout_error(remaining: std::time::Duration) -> ApiError {
    let secs = remaining.as_secs().max(1) as u32;
    ApiError(spiritstream_core::CoreError::RateLimited {
        retry_after_secs: secs,
    })
}

/// `POST /api/v1/auth/login` — validate token, set HttpOnly cookie.
///
/// Brute-force defense layered on top of the per-IP rate limit:
/// 1. If the account is currently locked (10 failures within 1h →
///    15min lockout), reply 429 with `Retry-After`. No timing work
///    happens so the rejection is cheap.
/// 2. Otherwise verify the token. On success, reset failure state and
///    set the session cookie.
/// 3. On failure, record the attempt; sleep for the resulting
///    exponential backoff before responding (so a pipelined attacker
///    can't bypass the delay by ignoring our response). If the failure
///    pushed the account into a fresh lockout, surface 429.
async fn auth_login(
    State(state): State<AppState>,
    cookies: Cookies,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthLoginResponse>, ApiError> {
    let account = AUTH_ACCOUNT_DEFAULT;

    if let Some(remaining) = state.auth_service.check_locked(account) {
        return Err(lockout_error(remaining));
    }

    let expected_token = state.auth_token.as_deref();
    let token_ok = match expected_token {
        None => true,
        Some(expected) => verify_token(expected, &payload.token),
    };

    if token_ok {
        state.auth_service.record_success(account);
        set_session_cookie(&state, &cookies);
        return Ok(Json(AuthLoginResponse {}));
    }

    log::warn!("auth_login: invalid token");
    match state.auth_service.record_failure(account) {
        Ok(backoff) => {
            // Sleep before returning so pipelined attackers can't outpace
            // the policy by ignoring our response timing.
            tokio::time::sleep(backoff).await;
            Err(ApiError(spiritstream_core::CoreError::Unauthorized))
        }
        Err(remaining) => Err(lockout_error(remaining)),
    }
}

/// POST /api/v1/auth/logout — drop session ID from active set, clear cookie.
async fn auth_logout(
    State(state): State<AppState>,
    cookies: Cookies,
) -> Result<Json<AuthLogoutResponse>, ApiError> {
    if let Some(c) = cookies.get(AUTH_COOKIE_NAME) {
        let mut sessions = state
            .active_sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        sessions.remove(c.value());
    }
    let cookie = Cookie::build((AUTH_COOKIE_NAME, ""))
        .path("/")
        .max_age(tower_cookies::cookie::time::Duration::ZERO)
        .build();
    cookies.remove(cookie);
    Ok(Json(AuthLogoutResponse {}))
}

/// `POST /api/v1/security/sessions/revoke-all` — drop every active
/// session ID. Existing cookies fail the next `auth_middleware` check
/// even though the client still possesses them. Requires a confirm
/// token issued for the `revoke_all_sessions` intent.
pub(crate) async fn security_revoke_all_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<RevokeAllSessionsResponse>, ApiError> {
    require_confirm_token(&state, &headers, "revoke_all_sessions")?;
    let mut sessions = state
        .active_sessions
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let revoked = sessions.len();
    sessions.clear();
    Ok(Json(RevokeAllSessionsResponse { revoked }))
}

/// GET /api/v1/auth/check — is the current session valid?
///
/// "Valid" means: cookie is present AND its value is in the
/// server-side active-session set. A revoked session keeps the raw cookie
/// in the browser but fails this check immediately.
async fn auth_check(
    State(state): State<AppState>,
    cookies: Cookies,
) -> Result<Json<AuthCheckResponse>, ApiError> {
    if state.auth_token.is_none() {
        return Ok(Json(AuthCheckResponse {
            authenticated: true,
            required: false,
        }));
    }
    let is_authenticated = match cookies.get(AUTH_COOKIE_NAME) {
        Some(c) => {
            let sessions = state
                .active_sessions
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            sessions.contains(c.value())
        }
        None => false,
    };
    Ok(Json(AuthCheckResponse {
        authenticated: is_authenticated,
        required: true,
    }))
}

// ============================================================================
// Middleware
// ============================================================================

/// Authentication middleware - check for valid session cookie
async fn auth_middleware(
    State(state): State<AppState>,
    cookies: Cookies,
    headers: HeaderMap,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    // If no token configured, allow all requests
    if state.auth_token.is_none() {
        return next.run(request).await;
    }

    // Cookie must exist AND be in the active-session set.
    // The MutexGuard MUST drop before any `await` or the future stops
    // being `Send`; we compute the predicate first, then await.
    let session_valid = cookies
        .get(AUTH_COOKIE_NAME)
        .map(|c| {
            let sessions = state
                .active_sessions
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            sessions.contains(c.value())
        })
        .unwrap_or(false);
    if session_valid {
        return next.run(request).await;
    }

    // Also accept Bearer token for backwards compatibility and programmatic access
    if let Some(token) = bearer_token(&headers) {
        if let Some(expected) = state.auth_token.as_deref() {
            if verify_token(expected, token) {
                return next.run(request).await;
            }
        }
    }

    // No valid session
    let response = InvokeResponse {
        ok: false,
        data: None,
        error: Some("Authentication required".to_string()),
    };
    (StatusCode::UNAUTHORIZED, Json(response)).into_response()
}

/// Per-endpoint rate limiting middleware.
///
/// Inspects the route and picks the corresponding `KeyedLimiter` from
/// [`EndpointRateLimiters`]. The key is the auth subject when an auth
/// cookie or Bearer token is present, otherwise the peer IP from
/// `ConnectInfo` (or `"unknown"` if the server is reached through a path
/// that doesn't surface `ConnectInfo` — primarily test harnesses).
///
/// Failures are returned as 429 with a JSON body matching the
/// `InvokeResponse` envelope so existing CLI parsers keep working.
async fn rate_limit_middleware(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    cookies: Cookies,
    headers: HeaderMap,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let path = request.uri().path();
    let method = request.method();

    let (limiter, key) = select_limiter(&state, path, method, &cookies, &headers, addr);

    match limiter.check_key(&key) {
        Ok(_) => next.run(request).await,
        Err(_) => {
            let response = InvokeResponse {
                ok: false,
                data: None,
                error: Some("Rate limit exceeded. Please try again later.".to_string()),
            };
            (StatusCode::TOO_MANY_REQUESTS, Json(response)).into_response()
        }
    }
}

/// Resolve the `(limiter, key)` pair for a request. Login uses peer IP
/// because the caller is by definition unauthenticated; every other
/// quota is keyed on the auth-subject hash so an attacker who steals one
/// token can't burn the entire shared budget.
fn select_limiter<'a>(
    state: &'a AppState,
    path: &str,
    method: &Method,
    cookies: &Cookies,
    headers: &HeaderMap,
    peer: SocketAddr,
) -> (&'a KeyedLimiter, String) {
    let peer_ip = peer.ip().to_string();
    let subject = subject_key(cookies, headers).unwrap_or_else(|| peer_ip.clone());

    if method == Method::POST && path == "/api/v1/auth/login" {
        return (&state.endpoint_limiters.login, peer_ip);
    }
    if method == Method::POST && path == "/api/v1/chat/messages" {
        return (&state.endpoint_limiters.chat_send, subject);
    }
    if method == Method::POST && path == "/api/v1/streams" {
        return (&state.endpoint_limiters.stream_start, subject);
    }
    if method == Method::POST && path.starts_with("/api/v1/oauth/") && path.ends_with("/flow") {
        return (&state.endpoint_limiters.oauth_flow, subject);
    }
    (&state.endpoint_limiters.default_auth, subject)
}

/// Derive a stable rate-limit subject key from the request's
/// authentication material. Returns `None` when no auth material is
/// present — callers fall back to peer IP. The key is a SHA-256 of the
/// token bytes (truncated for compactness) so the secret never appears
/// in the limiter's in-memory state.
fn subject_key(cookies: &Cookies, headers: &HeaderMap) -> Option<String> {
    use sha2::{Digest, Sha256};

    let token = cookies
        .get(AUTH_COOKIE_NAME)
        .map(|c| c.value().to_string())
        .or_else(|| bearer_token(headers).map(|t| t.to_string()))?;

    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let digest = hasher.finalize();
    Some(hex::encode(&digest[..16])) // 128-bit prefix is plenty for keying
}

/// CSRF guard.
///
/// Defends against cross-site request forgery on every state-changing
/// request (POST/PUT/PATCH/DELETE) and on WebSocket upgrades — a forged
/// upgrade equals a forged push channel, so we treat it the same as a
/// mutating request even though the underlying HTTP method is GET.
///
/// Policy:
/// 1. Pure-read methods (GET/HEAD/OPTIONS) without a WebSocket upgrade are
///    waved through — the browser will not run an authorized cross-site
///    mutation through them.
/// 2. `Sec-Fetch-Site` is the primary signal. `same-origin`, `same-site`,
///    and `none` (direct user action — typed URL, bookmark, redirect from
///    address bar) all pass. `cross-site` and `cross-origin` fall through
///    to the Origin allow-list so the Tauri 2 webview
///    (`tauri://localhost`, `https://tauri.localhost`) still works.
/// 3. The Origin allow-list is the secondary signal. It is also the only
///    check when `Sec-Fetch-Site` is absent (older browsers, CLI tools).
/// 4. Requests with neither header are allowed — these are CLI tools and
///    fall through to the existing `auth_middleware` token check.
///
/// Request-ID middleware. Generates a UUID v7 (time-ordered)
/// at the transport entry if the client didn't send one, then echoes
/// it back as `X-Request-Id` on the response. Downstream handlers can
/// read the value via the request extension; the value also flows into
/// `tracing` spans once the tracing migration runs.
async fn request_id_middleware(mut request: Request<axum::body::Body>, next: Next) -> Response {
    let header_name = "x-request-id";
    let incoming = request
        .headers()
        .get(header_name)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            if s.len() <= 64 && !s.is_empty() {
                Some(s.to_string())
            } else {
                None
            }
        });
    let rid = RequestId(incoming.unwrap_or_else(|| uuid::Uuid::now_v7().to_string()));
    // Store on the request extension so handlers can pull it out via
    // `Extension<RequestId>`.
    request.extensions_mut().insert(rid.clone());

    let mut response = next.run(request).await;
    // Echo the canonical id on the response by reading the struct
    // field through `RequestId::as_str` — the field accessor is the
    // public contract `Extension<RequestId>` consumers use too.
    if let Ok(value) = HeaderValue::from_str(rid.as_str()) {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}

/// Per-request correlation id. Constructed in `request_id_middleware`,
/// stored in the request extension, and echoed on the response via the
/// `X-Request-Id` header. Handlers extract it with `Extension<RequestId>`
/// when they want to thread the id into logs or audit entries.
#[derive(Debug, Clone)]
pub(crate) struct RequestId(pub String);

impl RequestId {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// Sources: OWASP CSRF cheat sheet (2024), MDN Sec-Fetch-Site.
async fn csrf_middleware(
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let headers = request.headers();

    let is_ws_upgrade = headers
        .get(header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);

    let is_state_changing = matches!(
        method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );

    if !is_state_changing && !is_ws_upgrade {
        return next.run(request).await;
    }

    let sec_fetch_site = headers.get("Sec-Fetch-Site").and_then(|v| v.to_str().ok());
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());

    let allowed = match sec_fetch_site {
        Some("same-origin") | Some("same-site") | Some("none") => true,
        Some(_) => origin
            .map(|o| origin_matches(o, &state.allowed_origins))
            .unwrap_or(false),
        None => match origin {
            Some(o) => origin_matches(o, &state.allowed_origins),
            None => true, // CLI tool — defer to auth_middleware
        },
    };

    if !allowed {
        let response = InvokeResponse {
            ok: false,
            data: None,
            error: Some("CSRF: request origin not allowed".to_string()),
        };
        return (StatusCode::FORBIDDEN, Json(response)).into_response();
    }

    next.run(request).await
}

// ============================================================================
// Request Handlers
// ============================================================================

// Health and readiness handlers live in `v1::v1_health` / `v1::v1_ready`.
// They are mounted at `/api/v1/health` and `/api/v1/ready` and that is the
// only place SpiritStream serves them — Tauri sidecar polling, Dockerfile
// healthcheck, and reverse-proxy configs all hit those URLs directly.

// ============================================================================
// File Browser Endpoints (for HTTP mode dialogs)
// ============================================================================

#[derive(Debug, Deserialize)]
struct FileBrowseQuery {
    path: Option<String>,
}

fn system_bin_paths() -> Vec<PathBuf> {
    if cfg!(target_os = "windows") {
        let mut paths = Vec::new();

        if let Some(program_files) = env::var_os("ProgramFiles") {
            paths.push(PathBuf::from(program_files));
        }
        if let Some(program_files_x86) = env::var_os("ProgramFiles(x86)") {
            paths.push(PathBuf::from(program_files_x86));
        }
        if let Some(program_data) = env::var_os("ProgramData") {
            let base = PathBuf::from(program_data);
            paths.push(base.clone());
            paths.push(base.join("chocolatey"));
            paths.push(base.join("chocolatey\\bin"));
        }
        if let Some(choco_install) = env::var_os("ChocolateyInstall") {
            let base = PathBuf::from(choco_install);
            paths.push(base.clone());
            paths.push(base.join("bin"));
        }
        if let Some(system_drive) = env::var_os("SystemDrive") {
            let drive = PathBuf::from(format!("{}\\", system_drive.to_string_lossy()));
            paths.push(drive.join("ffmpeg"));
            paths.push(drive.join("ffmpeg\\bin"));
            paths.push(drive.join("Windows\\System32"));
        }

        if paths.is_empty() {
            paths.push(PathBuf::from("C:\\Program Files"));
            paths.push(PathBuf::from("C:\\Program Files (x86)"));
        }

        return paths;
    }

    vec![
        PathBuf::from("/opt"),
        PathBuf::from("/usr/local"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/snap/bin"),
    ]
}

/// GET /api/files/browse - List directory contents
/// Query params: path (optional, defaults to home directory)
async fn files_browse(
    State(state): State<AppState>,
    Query(params): Query<FileBrowseQuery>,
) -> Result<Json<FileBrowseResponse>, ApiError> {
    use spiritstream_core::errors::ValidationIssue;
    use spiritstream_core::CoreError;

    let browse_path = match params.path {
        Some(p) if !p.is_empty() => PathBuf::from(&p),
        _ => state.home_dir.clone().ok_or_else(|| {
            ApiError(CoreError::Internal {
                context: "cannot determine home directory".into(),
            })
        })?,
    };

    let system_bin_paths = system_bin_paths();

    let mut allowed_dirs: Vec<&std::path::Path> = vec![state.app_data_dir.as_path()];
    if let Some(ref home) = state.home_dir {
        allowed_dirs.push(home.as_path());
    }
    for sys_path in &system_bin_paths {
        if sys_path.exists() {
            allowed_dirs.push(sys_path.as_path());
        }
    }

    validate_path_within_any(&browse_path, &allowed_dirs).map_err(ApiError)?;

    if !browse_path.exists() {
        return Err(ApiError(CoreError::ValidationFailed {
            reasons: vec![ValidationIssue {
                code: "directory_not_found".into(),
                message: format!("directory not found: {}", browse_path.display()),
                path: None,
            }],
        }));
    }

    if !browse_path.is_dir() {
        return Err(ApiError(CoreError::ValidationFailed {
            reasons: vec![ValidationIssue {
                code: "not_a_directory".into(),
                message: format!("path is not a directory: {}", browse_path.display()),
                path: None,
            }],
        }));
    }

    let entries = std::fs::read_dir(&browse_path).map_err(|e| {
        log::error!("Failed to read directory {browse_path:?}: {e}");
        ApiError(CoreError::Internal {
            context: format!("read_dir({}): {e}", browse_path.display()),
        })
    })?;

    let mut file_entries: Vec<FileEntry> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();

        if name.starts_with('.') {
            continue;
        }

        let metadata = entry.metadata().ok();
        let entry_type = if metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false) {
            "directory"
        } else {
            "file"
        };
        let size = if entry_type == "file" {
            metadata.as_ref().map(|m| m.len())
        } else {
            None
        };

        file_entries.push(FileEntry {
            name,
            entry_type: entry_type.to_string(),
            size,
        });
    }

    file_entries.sort_by(|a, b| match (&a.entry_type[..], &b.entry_type[..]) {
        ("directory", "file") => std::cmp::Ordering::Less,
        ("file", "directory") => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    let parent = browse_path.parent().and_then(|p| {
        let parent_path = p.to_path_buf();
        if validate_path_within_any(&parent_path, &allowed_dirs).is_ok() {
            Some(parent_path.to_string_lossy().to_string())
        } else {
            None
        }
    });

    Ok(Json(FileBrowseResponse {
        path: browse_path.to_string_lossy().to_string(),
        entries: file_entries,
        parent,
    }))
}

/// GET /api/files/home - Get user home directory path
async fn files_home(
    State(state): State<AppState>,
) -> Result<Json<FileHomeResponse>, ApiError> {
    use spiritstream_core::CoreError;

    let home = state.home_dir.as_ref().ok_or_else(|| {
        ApiError(CoreError::Internal {
            context: "cannot determine home directory".into(),
        })
    })?;

    Ok(Json(FileHomeResponse {
        path: home.to_string_lossy().to_string(),
    }))
}

#[derive(Debug, Deserialize)]
struct OpenPathRequest {
    path: String,
}

/// POST /api/v1/files/open — open path in the native file manager.
async fn files_open(
    State(state): State<AppState>,
    Json(payload): Json<OpenPathRequest>,
) -> Result<Json<FilesOpenResponse>, ApiError> {
    let path = PathBuf::from(&payload.path);

    let system_bin_paths = system_bin_paths();
    let mut allowed_dirs: Vec<&std::path::Path> = vec![state.app_data_dir.as_path()];
    if let Some(ref home) = state.home_dir {
        allowed_dirs.push(home.as_path());
    }
    for sys_path in &system_bin_paths {
        if sys_path.exists() {
            allowed_dirs.push(sys_path.as_path());
        }
    }

    validate_path_within_any(&path, &allowed_dirs).map_err(ApiError)?;

    if !path.exists() {
        return Err(ApiError(spiritstream_core::CoreError::NotFound {
            resource: format!("path: {}", path.display()),
        }));
    }

    opener::open(&path).map_err(|e| {
        log::error!("Failed to open path {path:?}: {e}");
        ApiError(spiritstream_core::CoreError::Internal {
            context: format!("opener::open({}): {e}", path.display()),
        })
    })?;

    Ok(Json(FilesOpenResponse {}))
}

#[derive(Debug, Deserialize)]
struct AuthQuery {
    token: Option<String>,
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    cookies: Cookies,
) -> impl IntoResponse {
    // Check authentication: no token required, valid cookie, or valid query param
    let authenticated = state.auth_token.is_none()
        || cookies.get(AUTH_COOKIE_NAME).is_some()
        || query.token.as_deref().is_some_and(|token| {
            state
                .auth_token
                .as_deref()
                .is_some_and(|expected| verify_token(expected, token))
        });

    if !authenticated {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }

    ws.on_upgrade(move |socket| handle_socket(socket, state.event_bus.subscribe()))
}

async fn handle_socket(mut socket: WebSocket, mut receiver: broadcast::Receiver<ServerEvent>) {
    while let Ok(event) = receiver.recv().await {
        if let Ok(payload) = serde_json::to_string(&event) {
            if socket.send(Message::Text(payload)).await.is_err() {
                break;
            }
        }
    }
}

// ============================================================================
// Main Entry Point
// ============================================================================

fn parse_host(host: &str) -> IpAddr {
    host.parse().unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}

/// Find themes directory by searching common relative paths from CWD.
/// Used as fallback when SPIRITSTREAM_THEMES_DIR is not set or invalid.
fn find_themes_dir_fallback() -> Option<String> {
    let cwd = std::env::current_dir().ok()?;

    let candidates = [
        cwd.join("themes"),
        cwd.join("../themes"),
        cwd.join("../../themes"),
        cwd.join("../../../themes"),
    ];

    for candidate in candidates {
        if let Ok(canonical) = candidate.canonicalize() {
            if canonical.is_dir() {
                // Verify it has theme files
                if std::fs::read_dir(&canonical)
                    .map(|entries| {
                        entries.flatten().any(|e| {
                            e.path()
                                .extension()
                                .map(|ext| ext == "jsonc" || ext == "json")
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
                {
                    return Some(canonical.to_string_lossy().to_string());
                }
            }
        }
    }
    None
}

fn init_logger(
    log_dir: &std::path::Path,
    event_bus: EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let logger = ServerLogger::new(log_dir, event_bus)?;
    log::set_boxed_logger(Box::new(logger))?;
    log::set_max_level(LevelFilter::Info);
    Ok(())
}

/// HTTP transport entrypoint. Invoked by the `spiritstream-server` binary
/// (and, in the future, by the Tauri 2 mobile shell when running the core
/// in-process).
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file if present (ignore if missing)
    dotenvy::dotenv().ok();

    // Load configuration from environment
    let data_dir = env::var("SPIRITSTREAM_DATA_DIR").unwrap_or_else(|_| "data".to_string());
    let log_dir = env::var("SPIRITSTREAM_LOG_DIR").unwrap_or_else(|_| format!("{data_dir}/logs"));
    // Resolve themes directory with fallback logic
    // Check if env var path exists and has theme files, otherwise try fallback paths
    let themes_dir = match env::var("SPIRITSTREAM_THEMES_DIR") {
        Ok(dir) => {
            let path = PathBuf::from(&dir);
            // Check if the path exists and has theme files
            let has_themes = std::fs::read_dir(&path)
                .map(|entries| {
                    entries.flatten().any(|e| {
                        e.path()
                            .extension()
                            .map(|ext| ext == "jsonc" || ext == "json")
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false);

            if has_themes {
                // Path exists and has theme files - use it
                // Will be logged after logger is initialized
                dir
            } else {
                // Path doesn't exist or has no themes - try fallback
                // Will be logged after logger is initialized
                if let Some(fallback) = find_themes_dir_fallback() {
                    fallback
                } else {
                    // No fallback found, use original (server will handle missing)
                    dir
                }
            }
        }
        Err(_) => {
            // Env var not set - try to find themes automatically
            if let Some(fallback) = find_themes_dir_fallback() {
                fallback
            } else {
                // Will be logged after logger is initialized
                "themes".to_string()
            }
        }
    };
    let ui_dir = env::var("SPIRITSTREAM_UI_DIR").unwrap_or_else(|_| "dist".to_string());
    // Host/port read from env vars (may be overridden by settings below)
    let env_host = env::var("SPIRITSTREAM_HOST").ok();
    let env_port: Option<u16> = env::var("SPIRITSTREAM_PORT")
        .ok()
        .and_then(|value| value.parse().ok());
    let env_auth_token = env::var("SPIRITSTREAM_API_TOKEN")
        .or_else(|_| env::var("SPIRITSTREAM_DEV_TOKEN"))
        .ok()
        .and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        });

    let app_data_dir = PathBuf::from(&data_dir);
    let log_dir_path = PathBuf::from(&log_dir);

    // Construct every service through the shared registry builder so that
    // transport-cli, the Tauri 2 mobile shell, and this HTTP transport all
    // wire services identically. Themes are configured after the load below.
    let event_bus = EventBus::new();
    let events_for_registry: Arc<dyn EventSink> = Arc::new(event_bus.clone());
    let registry =
        spiritstream_core::ServiceRegistry::build(spiritstream_core::ServiceRegistryOptions {
            data_dir: app_data_dir.clone(),
            themes_dir: PathBuf::from(&themes_dir),
            log_dir: log_dir_path.clone(),
            custom_ffmpeg_path: None, // populated below once settings are read
            events: events_for_registry,
        })
        .map_err(|e| -> Box<dyn std::error::Error> { format!("registry build: {e}").into() })?;
    let profile_manager = registry.profiles.clone();
    let settings_manager = registry.settings.clone();
    let theme_manager = registry.themes.clone();
    let ffmpeg_locator = registry.ffmpeg_locator.clone();
    let obs_handler = registry.obs.clone();
    let discord_service = registry.discord.clone();
    let chat_manager = registry.chat.clone();
    let oauth_service = registry.oauth.clone();

    // Load settings (global)
    let settings = settings_manager.load().ok();
    let last_profile = settings.as_ref().and_then(|s| s.last_profile.clone());

    // Load backend settings from the last active profile (per-profile integration)
    let mut backend_settings = ProfileSettings::default().backend;
    if let Some(profile_name) = last_profile.as_deref() {
        match profile_manager
            .load_with_key_decryption(profile_name, None)
            .await
        {
            Ok(profile) => {
                backend_settings = profile.settings.backend;
            }
            Err(err) => {
                log::warn!(
                    "Failed to load backend settings from profile '{}': {err}",
                    profile_name
                );
            }
        }
    }

    let settings_ui_enabled = backend_settings.ui_enabled;
    let env_ui_enabled = env::var("SPIRITSTREAM_UI_ENABLED")
        .ok()
        .and_then(|value| parse_bool(&value));
    let ui_enabled = env_ui_enabled.unwrap_or(settings_ui_enabled);
    let settings_auth_token = {
        let trimmed = backend_settings.token.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    };
    let auth_token = env_auth_token.or(settings_auth_token);

    // Cloud-mode startup guard. The server refuses to come
    // up when `SPIRITSTREAM_DEPLOY_MODE=cloud` if the deployment is
    // missing either a strong API token or an explicit declaration
    // that TLS termination lives in front of us (Caddy / Traefik /
    // nginx). The localhost dev path is untouched; this only fires
    // for operators standing up a public instance.
    let pre_deploy_mode = env::var("SPIRITSTREAM_DEPLOY_MODE").ok();
    if let Some(mode) = pre_deploy_mode.as_deref() {
        if mode.eq_ignore_ascii_case("cloud") {
            let tls_declared = env::var("SPIRITSTREAM_BEHIND_TLS_PROXY")
                .ok()
                .and_then(|v| parse_bool(&v))
                .unwrap_or(false);
            enforce_cloud_mode_preconditions(&auth_token, tls_declared)?;
        }
    }

    // Determine host/port: env vars take precedence, then settings, then defaults
    // If remote access is disabled in settings, force localhost regardless
    let (host, port) = {
        let remote_enabled = backend_settings.remote_enabled;
        let settings_host = backend_settings.host.clone();
        let settings_port = backend_settings.port;

        // Check if env var was explicitly set before consuming it
        let env_host_was_set = env_host.is_some();

        // Env var overrides settings, settings override defaults
        let configured_host = env_host.unwrap_or(settings_host);
        let configured_port = env_port.unwrap_or(settings_port);

        // If remote access is disabled, force localhost (unless env var explicitly set)
        let final_host = if !remote_enabled && !env_host_was_set {
            "127.0.0.1".to_string()
        } else {
            configured_host
        };

        (final_host, configured_port)
    };
    log::info!("Server will bind to {host}:{port}");

    let custom_ffmpeg_path = settings.as_ref().and_then(|s| {
        if s.ffmpeg_path.is_empty() {
            None
        } else {
            Some(s.ffmpeg_path.clone())
        }
    });

    if let Some(settings) = settings.as_ref() {
        let _ = prune_logs(&log_dir_path, settings.log_retention_days);
    }

    // Wire the FFmpeg handler against the user's optional custom path. The
    // registry already produced a default-pathed handler; replace it now
    // that we've loaded the settings that may override the binary location.
    let ffmpeg_handler = if custom_ffmpeg_path.is_some() {
        Arc::new(FFmpegHandler::new_with_custom_path(
            app_data_dir.clone(),
            custom_ffmpeg_path,
        ))
    } else {
        registry.ffmpeg.clone()
    };

    init_logger(&log_dir_path, event_bus.clone())?;

    // Log the themes directory configuration.
    let themes_path = PathBuf::from(&themes_dir);
    let themes_exist = themes_path.exists();
    let env_was_set = env::var("SPIRITSTREAM_THEMES_DIR").is_ok();
    log::info!("Themes directory: {themes_dir} (exists={themes_exist}, env_set={env_was_set})");
    if !themes_exist {
        log::warn!("Themes directory does not exist - custom themes may not load");
    }

    // Sync themes and verify sync worked.
    log::info!("Starting theme sync from {themes_dir:?} to user data");
    theme_manager.sync_project_themes();
    let synced_themes = theme_manager.list_themes();
    log::info!(
        "Theme sync complete. Available themes ({}): {:?}",
        synced_themes.len(),
        synced_themes.iter().map(|t| &t.id).collect::<Vec<_>>()
    );

    let theme_event_sink: Arc<dyn EventSink> = Arc::new(event_bus.clone());
    theme_manager.start_watcher(theme_event_sink);

    // Get home directory for path validation
    let home_dir = dirs_next::home_dir();

    // Select cookie mode from the resolved bind host + deploy mode.
    let deploy_mode = env::var("SPIRITSTREAM_DEPLOY_MODE").ok();
    let explicit_cookie_mode = env::var("SPIRITSTREAM_COOKIE_MODE").ok();
    let cookie_mode = SessionCookieMode::detect(
        &host,
        deploy_mode.as_deref(),
        explicit_cookie_mode.as_deref(),
    );
    log::info!("Cookie mode: {:?}", cookie_mode);

    // Origin allow-list shared with CORS.
    let allowed_origins = Arc::new(allowed_origins_from_env());

    // Per-endpoint keyed rate limiters replace the prior
    // single global governor quota. Brute-force defense.
    let endpoint_limiters = Arc::new(EndpointRateLimiters::from_env());
    let auth_service = Arc::new(AuthService::new());
    // One-shot confirm tokens + active-session registry.
    let confirm_tokens = Arc::new(ConfirmTokenService::new());
    let active_sessions = Arc::new(Mutex::new(std::collections::HashSet::new()));

    // OBS, Discord, Chat, OAuth, and FFmpegLocator all come from the
    // shared registry above (see `ServiceRegistry::build`).

    let readiness = Arc::new(ServerReadiness::default());

    let state = AppState {
        profile_manager,
        settings_manager,
        ffmpeg_handler,
        ffmpeg_locator,
        theme_manager,
        obs_handler,
        discord_service,
        chat_manager,
        oauth_service,
        event_bus,
        log_dir: log_dir_path,
        app_data_dir,
        auth_token,
        active_profile_name: Arc::new(AsyncMutex::new(None)),
        active_profile_settings: Arc::new(AsyncMutex::new(None)),
        active_profile_pii: Arc::new(AsyncMutex::new(None)),
        unlocked_profiles: Arc::new(AsyncMutex::new(std::collections::HashSet::new())),
        home_dir,
        cookie_mode,
        allowed_origins: allowed_origins.clone(),
        auth_service,
        endpoint_limiters,
        confirm_tokens,
        active_sessions,
        audit: registry.audit.clone(),
        safety: registry.safety.clone(),
        profile_activation: registry.profile_activation.clone(),
        readiness: readiness.clone(),
    };

    // Start background YouTube token refresh task
    start_youtube_token_refresh_task(state.clone()).await;

    // Backend-driven auto-retry on stream_error events. Frontend
    // used to do this; now the server owns both policy and trigger.
    start_auto_retry_task(state.clone()).await;

    // Start chat reconnect task (stream-tied)
    start_chat_reconnect_task(state.clone()).await;

    // Build CORS layer (shares the origin allow-list)
    let cors = build_cors_layer(allowed_origins.clone());

    // Build CSP header
    let csp_value = HeaderValue::from_static(
        "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; \
         connect-src 'self' ws://localhost:* wss://localhost:* http://localhost:* http://127.0.0.1:*; \
         img-src 'self' data:; font-src 'self'"
    );

    // Build router with security layers
    // Protected routes (require authentication)
    // Every route lives under /api/v1/* — no legacy aliases anywhere. Tauri
    // sidecar polling, Docker healthchecks, and reverse-proxy configs all
    // target /api/v1/health and /api/v1/ready directly. Frontends are
    // rewritten against the same surface.
    let protected_routes = Router::new()
        .route("/api/v1/files/browse", get(files_browse))
        .route("/api/v1/files/home", get(files_home))
        .route("/api/v1/files/open", post(files_open))
        .route("/api/v1/events", get(ws_handler))
        // Issue confirmation tokens for destructive ops.
        // The endpoint itself requires auth; the issued token is then
        // sent back as `X-Confirm-Token` on the destructive call.
        .route("/api/v1/security/confirm-token", post(confirm_token_issue))
        // Revoke every active session.
        .route(
            "/api/v1/security/sessions/revoke-all",
            post(security_revoke_all_sessions),
        )
        // Typed REST surface + `invoke` dispatch bridge (the bridge
        // is retired one command at a time as typed handlers replace it).
        .merge(v1::protected_router(state.clone()))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Public routes (no auth required).
    let public_routes = Router::new()
        .route("/api/v1/auth/login", post(auth_login))
        .route("/api/v1/auth/logout", post(auth_logout))
        .route("/api/v1/auth/check", get(auth_check))
        .merge(v1::public_router(state.clone()));

    // Combine all routes
    let mut app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .with_state(state.clone())
        // 2 MB JSON body cap is plenty for profile/settings
        // payloads. Per-route overrides (e.g. tighter caps on uploads)
        // can be applied at the individual handler with
        // `.layer(DefaultBodyLimit::max(N))`.
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        // CSRF guard runs before auth so a forged cross-site
        // mutation never even reaches the cookie / token check.
        .layer(middleware::from_fn_with_state(
            state.clone(),
            csrf_middleware,
        ))
        // Assign a request ID before any other middleware so
        // CSRF/auth/rate-limit rejections surface a useful identifier
        // for forensic correlation.
        .layer(middleware::from_fn(request_id_middleware))
        .layer(CookieManagerLayer::new())
        .layer(cors)
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CONTENT_SECURITY_POLICY,
            csp_value,
        ));

    // Optionally serve static UI files.
    //
    // Boot-coordination architecture: when readiness=false (services still
    // initializing), `GET /` returns a static loading page with
    // `<meta http-equiv="refresh" content="1">`. The browser auto-refreshes
    // every second until services initialize, at which point the same path
    // serves the SPA. This eliminates the "Could not connect" / 5xx cascade
    // in the browser console for Docker / cloud deployments — a user who
    // hits the URL before services are ready sees a styled loading page,
    // not a fetch error storm.
    //
    // Tauri shells don't load via `/`; they bundle the SPA and load via
    // `frontendDist` or `devUrl`. So this branch only matters when
    // `SPIRITSTREAM_UI_ENABLED=1` (Docker / cloud / server-bundled).
    let ui_path = PathBuf::from(ui_dir);
    if ui_enabled && ui_path.exists() {
        let index_path = ui_path.join("index.html");
        let readiness_for_root = readiness.clone();
        let index_for_root = index_path.clone();
        let root_handler = move || {
            let readiness = readiness_for_root.clone();
            let index_path = index_for_root.clone();
            async move {
                use std::sync::atomic::Ordering;
                if readiness.ready.load(Ordering::Acquire) {
                    match tokio::fs::read_to_string(&index_path).await {
                        Ok(html) => (
                            StatusCode::OK,
                            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                            html,
                        )
                            .into_response(),
                        Err(err) => {
                            log::error!("failed to read SPA index.html: {err}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "index.html missing".to_string(),
                            )
                                .into_response()
                        }
                    }
                } else {
                    let mut resp = (
                        StatusCode::SERVICE_UNAVAILABLE,
                        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                        LOADING_PAGE_HTML,
                    )
                        .into_response();
                    resp.headers_mut().insert(
                        header::RETRY_AFTER,
                        HeaderValue::from_static("1"),
                    );
                    resp
                }
            }
        };
        app = app
            .route("/", get(root_handler.clone()))
            .route("/index.html", get(root_handler))
            .fallback_service(
                ServeDir::new(&ui_path).fallback(ServeFile::new(ui_path.join("index.html"))),
            );
    }

    let address = SocketAddr::new(parse_host(&host), port);
    log::info!("SpiritStream backend listening on http://{address}");
    if state.auth_token.is_some() {
        log::info!("  Authentication: enabled");
    } else {
        log::info!("  Authentication: disabled (no token configured)");
    }

    let listener = tokio::net::TcpListener::bind(address).await?;

    // All services are constructed, background tasks are running, and the
    // TCP listener is accepting connections. Flip the readiness flag and
    // wake any clients parked on /api/v1/ready (the long-poll endpoint).
    // Any /ready request that arrives later sees the flag set and returns
    // 200 immediately without parking on the Notify.
    {
        use std::sync::atomic::Ordering;
        readiness.ready.store(true, Ordering::Release);
        readiness.notify.notify_waiters();
        log::info!("SpiritStream backend ready — readiness signal raised");
    }

    // `into_make_service_with_connect_info::<SocketAddr>()` is
    // required so the rate-limit middleware can extract the peer IP for
    // the login route (subject_key fallback when no auth is present).
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod phase_6_tests {
    use super::*;

    // ----- SessionCookieMode detection ----------------------------
    // `detect` is a pure function; tests pass the env override directly so
    // they remain parallel-safe (no global process-state mutation).

    #[test]
    fn cookie_mode_loopback_defaults_to_localhost_dev() {
        assert_eq!(
            SessionCookieMode::detect("127.0.0.1", None, None),
            SessionCookieMode::LocalhostDev,
        );
        assert_eq!(
            SessionCookieMode::detect("localhost", None, None),
            SessionCookieMode::LocalhostDev,
        );
        assert_eq!(
            SessionCookieMode::detect("::1", None, None),
            SessionCookieMode::LocalhostDev,
        );
    }

    #[test]
    fn cookie_mode_cloud_deploy_is_cross_origin() {
        // Cloud deploys must use SameSite=Lax (Strict drops the cookie
        // on cross-site navigation back to the UI).
        assert_eq!(
            SessionCookieMode::detect("0.0.0.0", Some("cloud"), None),
            SessionCookieMode::CrossOrigin,
        );
        assert_eq!(
            SessionCookieMode::detect("203.0.113.5", Some("CLOUD"), None),
            SessionCookieMode::CrossOrigin,
        );
    }

    #[test]
    fn cookie_mode_non_loopback_defaults_to_same_origin() {
        assert_eq!(
            SessionCookieMode::detect("192.168.1.50", Some("desktop"), None),
            SessionCookieMode::SameOrigin,
        );
        assert_eq!(
            SessionCookieMode::detect("server.local", None, None),
            SessionCookieMode::SameOrigin,
        );
    }

    #[test]
    fn cookie_mode_explicit_override_wins() {
        // Explicit override always wins, even when bind address + deploy
        // mode would auto-detect to something else.
        assert_eq!(
            SessionCookieMode::detect("127.0.0.1", None, Some("cross_origin")),
            SessionCookieMode::CrossOrigin,
        );
        assert_eq!(
            SessionCookieMode::detect("127.0.0.1", Some("cloud"), Some("same-origin")),
            SessionCookieMode::SameOrigin,
        );
        assert_eq!(
            SessionCookieMode::detect("public.example.com", Some("cloud"), Some("localhost-dev")),
            SessionCookieMode::LocalhostDev,
        );
        // Unrecognised override falls through to auto-detect.
        assert_eq!(
            SessionCookieMode::detect("127.0.0.1", None, Some("nonsense")),
            SessionCookieMode::LocalhostDev,
        );
    }

    #[test]
    fn cookie_mode_attributes_match_threat_model() {
        // SameOrigin: Strict + Secure
        assert!(SessionCookieMode::SameOrigin.secure());
        assert_eq!(
            SessionCookieMode::SameOrigin.same_site(),
            tower_cookies::cookie::SameSite::Strict,
        );
        // CrossOrigin: Lax + Secure
        assert!(SessionCookieMode::CrossOrigin.secure());
        assert_eq!(
            SessionCookieMode::CrossOrigin.same_site(),
            tower_cookies::cookie::SameSite::Lax,
        );
        // LocalhostDev: no Secure (would be rejected on plain HTTP), but
        // still Strict so even local malicious sites can't post.
        assert!(!SessionCookieMode::LocalhostDev.secure());
        assert_eq!(
            SessionCookieMode::LocalhostDev.same_site(),
            tower_cookies::cookie::SameSite::Strict,
        );
    }

    // ----- Origin allow-list pattern matching --------------------

    #[test]
    fn origin_matcher_wildcard_port() {
        let allowed = vec!["http://localhost:*".to_string()];
        assert!(origin_matches("http://localhost:5173", &allowed));
        assert!(origin_matches("http://localhost:8008", &allowed));
        // Bare "http://localhost" (no port) does NOT match `:*` — the
        // wildcard requires a port to be present.
        assert!(!origin_matches("http://localhost", &allowed));
        // Different host does not match.
        assert!(!origin_matches("http://evil.com:5173", &allowed));
    }

    #[test]
    fn origin_matcher_exact_match() {
        let allowed = vec![
            "tauri://localhost".to_string(),
            "https://tauri.localhost".to_string(),
        ];
        assert!(origin_matches("tauri://localhost", &allowed));
        assert!(origin_matches("https://tauri.localhost", &allowed));
        assert!(!origin_matches("http://tauri.localhost", &allowed));
        assert!(!origin_matches("tauri://evil.com", &allowed));
    }

    #[test]
    fn origin_matcher_rejects_substring_attacks() {
        // An attacker registering `localhost.evil.com` must not match
        // `http://localhost:*` via prefix string matching.
        let allowed = vec!["http://localhost:*".to_string()];
        assert!(!origin_matches("http://localhost.evil.com:5173", &allowed));
    }

    // ----- mask_sensitive coverage + proptest -----------------

    #[test]
    fn mask_sensitive_redacts_rtmp_stream_key() {
        let log = "Starting stream to rtmp://live.twitch.tv/app/live_12345_abcdefghijklmnop";
        let masked = mask_sensitive(log);
        assert!(
            !masked.contains("live_12345_abcdefghijklmnop"),
            "stream key leaked: {masked}"
        );
        assert!(masked.contains("[REDACTED]"));
    }

    #[test]
    fn mask_sensitive_redacts_template_expansion() {
        let log = "ffmpeg -f flv rtmp://server.example.com/app/${STREAM_KEY}";
        let masked = mask_sensitive(log);
        assert!(
            !masked.contains("${STREAM_KEY}"),
            "template var leaked: {masked}"
        );
        assert!(masked.contains("[REDACTED]"));
    }

    #[test]
    fn mask_sensitive_redacts_bearer_token() {
        let log = "Authorization: Bearer abcdefghijklmnopqrstuvwxyz1234567890";
        let masked = mask_sensitive(log);
        assert!(
            !masked.contains("abcdefghijklmnopqrstuvwxyz1234567890"),
            "bearer token leaked: {masked}",
        );
    }

    #[test]
    fn mask_sensitive_redacts_enc_v1_and_v2_blobs() {
        let v1 = "ENC::dGVzdHRlc3R0ZXN0dGVzdA==";
        let v2 = "ENC2::aGVsbG93b3JsZGhlbGxvd29ybGQ=";
        let masked_v1 = mask_sensitive(&format!("setting={v1}"));
        let masked_v2 = mask_sensitive(&format!("setting={v2}"));
        assert!(
            masked_v1.contains("[ENCRYPTED]"),
            "v1 blob not masked: {masked_v1}"
        );
        assert!(
            masked_v2.contains("[ENCRYPTED]"),
            "v2 blob not masked: {masked_v2}"
        );
    }

    proptest::proptest! {
        /// Property: any token-shaped string (≥20 chars of
        /// `[A-Za-z0-9_\-./+]`) that follows a recognised keyword like
        /// `token=`, `bearer `, `password:`, etc. MUST be redacted from
        /// the output of `mask_sensitive`. Fuzz coverage of this
        /// property is required to catch new token shapes.
        #[test]
        fn prop_token_after_keyword_is_always_redacted(
            keyword in "token|key|password|secret|bearer|oauth|access_token|refresh_token|authorization",
            separator in "[:=]| ",
            token in "[A-Za-z0-9_\\-./+]{20,80}",
            prefix in "[a-z ]{0,30}",
            suffix in "[a-z ]{0,30}",
        ) {
            let input = format!("{prefix}{keyword}{separator}{token}{suffix}");
            let masked = mask_sensitive(&input);
            proptest::prop_assert!(
                !masked.contains(&token),
                "token leaked after keyword {keyword:?}: input={input:?}, masked={masked:?}",
            );
        }

        /// Property: any RTMP URL with a trailing path segment of 1+ char
        /// must have that segment masked. The segment is the stream key
        /// in both Twitch and YouTube URLs.
        #[test]
        fn prop_rtmp_path_segment_is_always_redacted(
            scheme in "rtmps?",
            host in "[a-z]{3,12}\\.[a-z]{3,5}",
            app in "[a-z]{3,12}",
            key in "[a-zA-Z0-9_]{8,40}",
        ) {
            let input = format!("{scheme}://{host}/{app}/{key}");
            let masked = mask_sensitive(&input);
            proptest::prop_assert!(
                !masked.contains(&key) || masked.contains("[REDACTED]"),
                "rtmp stream key leaked: input={input:?}, masked={masked:?}",
            );
        }
    }

    // ----- cloud-mode startup guard ----------------------------
    // `enforce_cloud_mode_preconditions` is pure (takes the
    // tls-declared flag as a parameter, not from env) so these tests
    // are parallel-safe without `serial_test`.

    #[test]
    fn cloud_mode_refuses_to_start_without_strong_token() {
        let weak = Some("short".to_string());
        let err = enforce_cloud_mode_preconditions(&weak, true).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("SPIRITSTREAM_API_TOKEN"),
            "expected token error: {msg}"
        );
        assert!(msg.contains("32"), "expected min-length hint: {msg}");
    }

    #[test]
    fn cloud_mode_refuses_to_start_without_token_at_all() {
        let none: Option<String> = None;
        let err = enforce_cloud_mode_preconditions(&none, true).unwrap_err();
        assert!(format!("{err}").contains("SPIRITSTREAM_API_TOKEN"));
    }

    #[test]
    fn cloud_mode_refuses_to_start_without_tls_proxy_declared() {
        let strong = Some("a".repeat(32));
        let err = enforce_cloud_mode_preconditions(&strong, false).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("SPIRITSTREAM_BEHIND_TLS_PROXY"),
            "expected TLS-proxy error: {msg}",
        );
    }

    #[test]
    fn cloud_mode_starts_when_both_preconditions_satisfied() {
        let strong = Some("0123456789abcdef0123456789abcdef".to_string());
        let result = enforce_cloud_mode_preconditions(&strong, true);
        assert!(result.is_ok(), "expected Ok, got: {result:?}");
    }

    #[test]
    fn cloud_mode_accepts_exactly_32_char_token() {
        // Boundary check — the policy says "≥ 32 chars".
        let exactly_32 = Some("a".repeat(32));
        assert!(enforce_cloud_mode_preconditions(&exactly_32, true).is_ok());
    }

    #[test]
    fn cloud_mode_rejects_31_char_token() {
        let just_under = Some("a".repeat(31));
        assert!(enforce_cloud_mode_preconditions(&just_under, true).is_err());
    }
}
