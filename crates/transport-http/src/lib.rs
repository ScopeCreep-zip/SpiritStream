use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        DefaultBodyLimit, Query, State,
    },
    http::{header, HeaderValue, StatusCode},
    middleware::{from_fn, from_fn_with_state},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    env,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::sync::{broadcast, Mutex as AsyncMutex};
use tower_cookies::{CookieManagerLayer, Cookies};
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
};

use spiritstream_core::models::ProfileSettings;
use spiritstream_core::services::{
    prune_logs, validate_path_within_any, AuditLogService, AuthService,
    ChatManager, ConfirmTokenService, DiscordWebhookService, EventSink, FFmpegHandler,
    FFmpegLocator, OAuthService, ObsWebSocketHandler, ProfileManager, SafetyService,
    SettingsManager, ThemeManager,
};

// Versioned REST API surface (`/api/v1/*`). New typed handlers live here.
// See `crates/transport-http/src/v1.rs` and the rewrite plan.
pub mod v1;

mod error;
pub use error::ApiError;

mod transport;
pub use transport::HttpTransport;

// Focused submodules — extracted from this file as part of the
// `crates/transport-http/src/lib.rs` god-class split.
mod auth;
mod auth_helpers;
mod chat_lifecycle;
mod cloud_mode;
mod cors;
mod events;
mod file_browser;
mod logger;
mod middleware;
mod rate_limit;
mod redaction;
mod session;

use auth::{auth_check, auth_login, auth_logout, confirm_token_issue, security_revoke_all_sessions};
pub(crate) use auth::require_confirm_token;
use auth_helpers::verify_token;
use chat_lifecycle::{
    start_auto_retry_task, start_chat_reconnect_task, start_youtube_token_refresh_task,
};
pub(crate) use chat_lifecycle::{
    auto_connect_chat_platforms, auto_disconnect_chat_platforms, build_hour_keys,
    clear_profile_oauth_account, connect_trovo_chat, connect_twitch_chat,
    connect_youtube_chat_with_retry, ensure_fresh_oauth_token, get_active_profile_name,
    get_active_profile_settings, persist_active_profile_settings, set_active_profile,
    update_profile_oauth_account,
};
use cloud_mode::{enforce_cloud_mode_preconditions, parse_bool};
#[cfg(test)]
use cors::origin_matches;
use cors::{allowed_origins_from_env, build_cors_layer};
use events::{EventBus, ServerEvent};
use logger::init_logger;
use file_browser::{files_browse, files_home, files_open};
use middleware::{auth_middleware, csrf_middleware, rate_limit_middleware, request_id_middleware};
use rate_limit::EndpointRateLimiters;
#[cfg(test)]
use redaction::mask_sensitive;
use redaction::redact_payload;
use session::SessionCookieMode;

// ============================================================================
// Constants
// ============================================================================

const AUTH_COOKIE_NAME: &str = "spiritstream_session";
const COOKIE_MAX_AGE_SECS: i64 = 7 * 24 * 60 * 60; // 7 days
const DEFAULT_RATE_LIMIT_PER_MINUTE: u32 = 300;

// ============================================================================
// Application State
// ============================================================================

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


// FilesOpenResponse is consumed by `file_browser::files_open` but defined here
// (and exposed via `crate::FilesOpenResponse`) so v1 OpenAPI tooling can see
// the type at the crate root.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct FilesOpenResponse {}

// ============================================================================
// Request Handlers
// ============================================================================

// Health and readiness handlers live in `v1::v1_health` / `v1::v1_ready`.
// They are mounted at `/api/v1/health` and `/api/v1/ready` and that is the
// only place SpiritStream serves them — Tauri sidecar polling, Dockerfile
// healthcheck, and reverse-proxy configs all hit those URLs directly.

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
        .layer(from_fn_with_state(
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
        .layer(from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        // CSRF guard runs before auth so a forged cross-site
        // mutation never even reaches the cookie / token check.
        .layer(from_fn_with_state(
            state.clone(),
            csrf_middleware,
        ))
        // Assign a request ID before any other middleware so
        // CSRF/auth/rate-limit rejections surface a useful identifier
        // for forensic correlation.
        .layer(from_fn(request_id_middleware))
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
