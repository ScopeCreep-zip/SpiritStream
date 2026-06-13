use axum::{
    extract::DefaultBodyLimit,
    http::{header, HeaderValue},
    middleware::{from_fn, from_fn_with_state},
    routing::{get, post},
    Router,
};
use serde::Serialize;
use serde_json::Value;
use std::{env, net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::sync::Mutex as AsyncMutex;
use tower_cookies::CookieManagerLayer;
use tower_http::set_header::SetResponseHeaderLayer;

use spiritstream_core::models::ProfileSettings;
use spiritstream_core::services::{
    prune_logs, validate_path_within_any, AuditLogService, AuthService, ChatManager,
    ConfirmTokenService, DiscordWebhookService, EventSink, FFmpegHandler, FFmpegLocator,
    OAuthService, ObsWebSocketHandler, ProfileManager, SafetyService, SettingsManager,
    ThemeManager,
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

pub(crate) use auth::require_confirm_token;
use auth::{
    auth_check, auth_login, auth_logout, confirm_token_issue, events_ticket_issue,
    security_revoke_all_sessions,
};
pub(crate) use chat_lifecycle::{
    auto_connect_chat_platforms, build_hour_keys,
    clear_profile_oauth_account, connect_trovo_chat, connect_twitch_chat,
    connect_youtube_chat_with_retry, ensure_fresh_oauth_token, get_active_profile_name,
    get_active_profile_settings, persist_active_profile_settings, set_active_profile,
    update_profile_oauth_account,
};
use chat_lifecycle::{
    start_auto_retry_task, start_chat_reconnect_task, start_youtube_token_refresh_task,
};
use cloud_mode::{enforce_cloud_mode_preconditions, parse_bool};
#[cfg(test)]
use cors::origin_matches;
use cors::{allowed_origins_from_env, build_cors_layer};
use events::EventBus;
use file_browser::{files_browse, files_home, files_open};
use logger::init_logger;
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
    /// Server-tracked sessions — cross-process, file-backed, hashed
    /// (`core::services::SessionStore`). Cookies whose value is not in
    /// the store fail auth, so a revoke — from THIS process or from
    /// `spiritstream-cli session revoke-all` — invalidates every
    /// existing session on its next request.
    pub(crate) sessions: Arc<spiritstream_core::services::SessionStore>,
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
    /// The full wired registry. Handlers that need the cross-transport
    /// orchestration helpers (e.g. `rotate_machine_key_checked`) call
    /// through this instead of re-implementing the rules locally.
    pub(crate) registry: spiritstream_core::ServiceRegistry,
    /// Trusted reverse-proxy CIDRs (`SPIRITSTREAM_TRUSTED_PROXIES`).
    /// When the direct peer is one of these, rate-limit keying reads
    /// the rightmost-untrusted `X-Forwarded-For` hop as the client.
    pub(crate) trusted_proxies: Arc<Vec<ipnet::IpNet>>,
    /// One-shot tickets authenticating the `/api/v1/events` WebSocket
    /// upgrade (browsers can't send headers there; cross-origin
    /// deployments don't send the cookie either).
    pub(crate) event_tickets: Arc<spiritstream_core::services::EventTicketService>,
}

/// Error envelope for middleware-layer rejections (auth, CSRF, rate
/// limit) that fire before a typed handler is reached. Handlers
/// themselves use the `{ kind, details }` `ApiError` contract.
#[derive(Serialize)]
struct MiddlewareErrorResponse {
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

// K4: WS handler + small env-resolution helpers live in `lib/helpers.rs`
// so this orchestrator stays under the 600 LOC ceiling.
#[path = "lib/helpers.rs"]
mod helpers;
#[path = "lib/port_file.rs"]
mod port_file;
#[path = "lib/static_ui.rs"]
mod static_ui;
use helpers::{find_themes_dir_fallback, parse_host, ws_handler};
use port_file::{remove_port_file, resolve_bind_port, write_port_file};

/// HTTP transport entrypoint. Invoked by the `spiritstream-server` binary
/// (and, in the future, by the Tauri 2 mobile shell when running the core
/// in-process).
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file if present (ignore if missing)
    dotenvy::dotenv().ok();

    // Load configuration from environment. `env::var` returns Err only
    // when the variable is *missing*; an explicitly-empty value (e.g.
    // `SPIRITSTREAM_DATA_DIR=`) returns `Ok("")` and would silently
    // create / look up an empty-string path. Treat empty as missing
    // so misconfigured deploys fall back to the documented defaults.
    let data_dir = env::var("SPIRITSTREAM_DATA_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "data".to_string());
    let log_dir = env::var("SPIRITSTREAM_LOG_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("{data_dir}/logs"));
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
    let ui_dir = env::var("SPIRITSTREAM_UI_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "dist".to_string());
    // Host/port read from env vars (may be overridden by settings below)
    let env_host = env::var("SPIRITSTREAM_HOST").ok();
    // H8: accept 0 ("ask the OS for any free port" — the bound port is
    // discoverable afterwards via `run/server.port` and the startup
    // log) or an explicit unprivileged port. Privileged ports (1..1024)
    // need root and are typically wrong for a user-mode server; reject
    // them so a misconfiguration surfaces loudly.
    let env_port: Option<u16> = match env::var("SPIRITSTREAM_PORT") {
        Ok(value) => match value.parse::<u16>() {
            Ok(n) if n == 0 || (1024..=65535).contains(&n) => Some(n),
            Ok(other) => {
                return Err(format!(
                    "SPIRITSTREAM_PORT={other} is out of range; pick 0 (OS-assigned) or a port in 1024..=65535"
                )
                .into());
            }
            Err(e) => {
                return Err(format!("SPIRITSTREAM_PORT={value:?} is not a valid u16: {e}").into());
            }
        },
        Err(_) => None,
    };
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
    let secret_store_override = env::var("SPIRITSTREAM_SECRET_STORE").ok();
    let secret_store = spiritstream_core::services::build_secret_store(
        &app_data_dir,
        secret_store_override.as_deref(),
    )
    .map_err(|e| -> Box<dyn std::error::Error> {
        format!("secret store selection: {e}").into()
    })?;
    let registry =
        spiritstream_core::ServiceRegistry::build(spiritstream_core::ServiceRegistryOptions {
            data_dir: app_data_dir.clone(),
            themes_dir: PathBuf::from(&themes_dir),
            log_dir: log_dir_path.clone(),
            custom_ffmpeg_path: None, // populated below once settings are read
            events: events_for_registry,
            secret_store,
        })
        .map_err(|e| -> Box<dyn std::error::Error> { format!("registry build: {e}").into() })?;
    // Restore client credentials the user saved through the in-app
    // setup form. Failure is logged loudly but doesn't abort startup:
    // the affected providers report unconfigured, which the UI shows
    // honestly — locking the user out of the whole app over optional
    // sign-in config would be the worse failure for this population.
    if let Err(e) = registry.oauth.load_persisted().await {
        log::error!("failed to load stored OAuth client credentials: {e}");
    }
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
    // Trusted reverse-proxy CIDRs for proxy-aware client-IP extraction
    // (rate-limit keying). Invalid entries are a startup error — a
    // typo'd CIDR silently disabling proxy awareness would put every
    // client back in one shared login bucket.
    let trusted_proxies: Vec<ipnet::IpNet> = match env::var("SPIRITSTREAM_TRUSTED_PROXIES") {
        Ok(raw) if !raw.trim().is_empty() => raw
            .split(',')
            .map(|entry| {
                let entry = entry.trim();
                entry.parse::<ipnet::IpNet>().or_else(|_| {
                    entry
                        .parse::<std::net::IpAddr>()
                        .map(ipnet::IpNet::from)
                        .map_err(|_| ())
                })
                .map_err(|_| -> Box<dyn std::error::Error> {
                    format!(
                        "SPIRITSTREAM_TRUSTED_PROXIES entry {entry:?} is not a valid IP or CIDR"
                    )
                    .into()
                })
            })
            .collect::<Result<_, _>>()?,
        _ => Vec::new(),
    };

    let pre_deploy_mode = env::var("SPIRITSTREAM_DEPLOY_MODE").ok();
    if let Some(mode) = pre_deploy_mode.as_deref() {
        if mode.eq_ignore_ascii_case("cloud") {
            let tls_declared = env::var("SPIRITSTREAM_BEHIND_TLS_PROXY")
                .ok()
                .and_then(|v| parse_bool(&v))
                .unwrap_or(false);
            enforce_cloud_mode_preconditions(&auth_token, tls_declared, !trusted_proxies.is_empty())?;
        }
    }

    // Determine host/port. Host: env wins, then settings — and when
    // remote access is off, localhost is forced unless env explicitly
    // overrides. Port: env wins (including an explicit 0); otherwise
    // the profile port applies only with remote access ON. Remote OFF
    // means the OS assigns a free port — see `resolve_bind_port`.
    let (host, port) = {
        let remote_enabled = backend_settings.remote_enabled;
        let settings_host = backend_settings.host.clone();

        let env_host_was_set = env_host.is_some();
        let configured_host = env_host.unwrap_or(settings_host);
        let final_host = if !remote_enabled && !env_host_was_set {
            "127.0.0.1".to_string()
        } else {
            configured_host
        };

        let final_port = resolve_bind_port(env_port, remote_enabled, backend_settings.port);
        (final_host, final_port)
    };
    if port == 0 {
        // A reverse proxy (Caddy/nginx/ingress) needs a static upstream;
        // an OS-assigned port behind one is a misconfiguration, not a
        // convenience. Fail loud instead of binding somewhere the proxy
        // will never find.
        if pre_deploy_mode
            .as_deref()
            .is_some_and(|m| m.eq_ignore_ascii_case("cloud"))
        {
            return Err(
                "cloud mode requires a fixed port: set SPIRITSTREAM_PORT to the port your \
                 reverse proxy targets (an OS-assigned port 0 is only valid for desktop/dev)"
                    .into(),
            );
        }
        log::info!("Server will bind to {host}:<OS-assigned> (port 0 requested)");
    } else {
        log::info!("Server will bind to {host}:{port}");
    }

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
        )?)
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
    // Q6: ConfirmTokenService comes from the shared registry so a
    // token issued via `spiritstream-cli confirm-token issue --intent`
    // is consumable on the HTTP destructive endpoints (and vice versa).
    // Pre-Q6 the HTTP transport constructed its own instance, which
    // made the CLI subcommand decoratively useless — issued tokens
    // never reached the HTTP validator.
    let confirm_tokens = registry.confirm_tokens.clone();
    let sessions = registry.sessions.clone();
    let event_tickets = Arc::new(spiritstream_core::services::EventTicketService::new());

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
        sessions,
        audit: registry.audit.clone(),
        safety: registry.safety.clone(),
        profile_activation: registry.profile_activation.clone(),
        readiness: readiness.clone(),
        registry: registry.clone(),
        trusted_proxies: Arc::new(trusted_proxies),
        event_tickets,
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

    // Build CSP header. `frame-ancestors 'none'` is the CSP-level
    // equivalent of `X-Frame-Options: DENY` — both are emitted (H3)
    // because some browsers (Safari pre-15.4) ignore one of them.
    //
    // `style-src` carries NO `'unsafe-inline'` (M3): the SPA's only inline
    // style moved to /boot.css and custom-theme tokens apply via CSSOM, while
    // the static loading page's one inline `<style>` is allow-listed by its
    // SHA-256 hash. `font-src`/`style-src` intentionally omit the Google Fonts
    // hosts so the Docker/browser path stays self-only (privacy threat model);
    // bundled fonts fall back to the system stack there.
    let csp_value = {
        let style_hash = static_ui::loading_page_style_csp_hash();
        let csp = format!(
            "default-src 'self'; script-src 'self'; style-src 'self' {style_hash}; \
             connect-src 'self' ws://localhost:* wss://localhost:* http://localhost:* http://127.0.0.1:*; \
             img-src 'self' data:; font-src 'self'; frame-ancestors 'none'"
        );
        HeaderValue::from_str(&csp).expect("CSP is valid header ASCII")
    };

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
        // One-shot tickets for the events WebSocket (the upgrade itself
        // is mounted below with its own cookie-or-ticket check).
        .route("/api/v1/events/ticket", post(events_ticket_issue))
        // Issue confirmation tokens for destructive ops.
        // The endpoint itself requires auth; the issued token is then
        // sent back as `X-Confirm-Token` on the destructive call.
        .route("/api/v1/security/confirm-token", post(confirm_token_issue))
        // Revoke every active session.
        .route(
            "/api/v1/security/sessions/revoke-all",
            post(security_revoke_all_sessions),
        )
        // Typed REST surface (`/api/v1/*`).
        .merge(v1::protected_router(state.clone()))
        // H2: rate-limit AFTER auth for protected routes. Pre-H2 the
        // limiter sat on the global stack, so unauthenticated traffic
        // to a protected endpoint still counted against the per-IP
        // budget — an attacker could exhaust legit users' quotas
        // without ever holding a credential. Now the rate limit only
        // ticks once auth_middleware has admitted the request.
        .layer(from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        .layer(from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Public routes (no auth required). Login MUST still be
    // IP-rate-limited (the limiter dispatcher selects the login bucket
    // for `POST /api/v1/auth/login`) so brute-force attempts hit a
    // ceiling even without an auth subject.
    let public_routes = Router::new()
        .route("/api/v1/auth/login", post(auth_login))
        .route("/api/v1/auth/logout", post(auth_logout))
        .route("/api/v1/auth/check", get(auth_check))
        .merge(v1::public_router(state.clone()))
        .layer(from_fn_with_state(state.clone(), rate_limit_middleware));

    // The events WebSocket lives OUTSIDE auth_middleware: the upgrade
    // authenticates via session cookie (same-origin shells) OR a
    // one-shot ticket from /api/v1/events/ticket (cross-origin
    // browsers, which can neither send headers nor — under
    // SameSite=Lax — the cookie on a WS upgrade). ws_handler enforces
    // that predicate itself; CSRF still guards the upgrade at the
    // global layer.
    let events_routes = Router::new()
        .route("/api/v1/events", get(ws_handler))
        .with_state(state.clone());

    // Combine all routes. NOTE: nothing may be `.route()`d after the
    // `.layer(...)` stack below — axum layers wrap only the routes that
    // exist when `.layer` is called, so anything added later ships with
    // ZERO security headers. The static UI used to be mounted after the
    // layers for exactly that reason: the self-hosted SPA document and
    // every JS/CSS asset went out without CSP, X-Frame-Options,
    // X-Content-Type-Options, or Referrer-Policy.
    let mut app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .with_state(state.clone())
        .merge(events_routes);

    // Static UI (Docker / cloud / server-bundled SPA). Mounted BEFORE the
    // layer stack — see the layering note above — so the document and
    // every asset carry the security headers. Implementation lives in
    // `lib/static_ui.rs`.
    let ui_path = PathBuf::from(ui_dir);
    app = static_ui::mount_static_ui(app, ui_enabled, &ui_path, readiness.clone());

    let app = app
        // 2 MB JSON body cap is plenty for profile/settings
        // payloads. Per-route overrides (e.g. tighter caps on uploads)
        // can be applied at the individual handler with
        // `.layer(DefaultBodyLimit::max(N))`.
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
        // CSRF guard runs before auth so a forged cross-site
        // mutation never even reaches the cookie / token check.
        // Safe methods pass straight through, so static GETs are
        // unaffected by it.
        .layer(from_fn_with_state(state.clone(), csrf_middleware))
        // Assign a request ID before any other middleware so
        // CSRF/auth/rate-limit rejections surface a useful identifier
        // for forensic correlation.
        .layer(from_fn(request_id_middleware))
        .layer(CookieManagerLayer::new())
        .layer(cors)
        // H3: defense-in-depth response headers. Pre-H3 only CSP was
        // emitted; missing X-Content-Type-Options enabled MIME
        // sniffing attacks on user-controlled binary attachments,
        // missing X-Frame-Options allowed clickjacking iframes,
        // missing Referrer-Policy leaked full paths to outbound link
        // targets. The CSP itself now also names `frame-ancestors`
        // (set in `csp_value` below) for browsers that ignore XFO.
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CONTENT_SECURITY_POLICY,
            csp_value,
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::REFERRER_POLICY,
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ));

    let address = SocketAddr::new(parse_host(&host), port);
    let listener = tokio::net::TcpListener::bind(address).await?;
    // Log AFTER binding with the listener's own address — with port 0
    // the requested address would read ":0"; `local_addr()` carries the
    // port the OS actually assigned.
    let bound_addr = listener.local_addr()?;
    log::info!("SpiritStream backend listening on http://{bound_addr}");
    if state.auth_token.is_some() {
        log::info!("  Authentication: enabled");
    } else {
        log::info!("  Authentication: disabled (no token configured)");
    }

    // Persist the bound port for cross-process discovery (the Tauri
    // shell health-checks the sidecar through this file; it is the
    // single source of truth for "where did the server come up").
    write_port_file(&state.app_data_dir, bound_addr.port())?;

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

    // G2: record startup in the HMAC chain. If the chain is already
    // tampered, record that observation too so operators see when the
    // tamper was first noticed by a fresh process boot.
    let _ = registry
        .audit
        .record(spiritstream_core::services::AuditAction::AppStarted);
    if let Ok(spiritstream_core::services::AuditChainStatus::Tampered {
        last_valid_sequence,
        ..
    }) = registry.audit.verify_chain()
    {
        let _ = registry.audit.record(
            spiritstream_core::services::AuditAction::AuditLogTamperDetected {
                last_valid_sequence,
            },
        );
    }

    // `into_make_service_with_connect_info::<SocketAddr>()` is
    // required so the rate-limit middleware can extract the peer IP for
    // the login route (subject_key fallback when no auth is present).
    let serve_result = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await;

    // G2: record stop after axum::serve returns (orderly shutdown).
    let _ = registry
        .audit
        .record(spiritstream_core::services::AuditAction::AppStopped);
    remove_port_file(&state.app_data_dir);

    serve_result?;
    Ok(())
}

#[cfg(test)]
#[path = "lib/tests.rs"]
mod phase_6_tests;
