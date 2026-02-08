use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Json, Path, Query, State,
    },
    http::{header, HeaderMap, HeaderValue, Method, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use log::{Level, LevelFilter, Log, Metadata, Record};
use chrono::{DateTime, Duration, Local, TimeZone, Timelike};
use std::{
    collections::HashMap,
    env,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, BufWriter, Write},
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

use spiritstream_server::commands::{get_encoders, test_ffmpeg, test_rtmp_target, validate_ffmpeg_path};
use spiritstream_server::models::{OutputGroup, Profile, RtmpInput, Settings, ObsIntegrationDirection, ChatConfig, ChatPlatform, ChatCredentials, TwitchAuth, YouTubeAuth, ChatMessage, ChatSendResult};
use spiritstream_server::services::{
    prune_logs, read_recent_logs, validate_extension, validate_path_within_any,
    ChatManager, DiscordWebhookService, Encryption, EventSink, FFmpegDownloader, FFmpegHandler,
    OAuthCallback, OAuthCallbackServer, OAuthConfig, OAuthService, ObsConfig, ObsWebSocketHandler,
    ProfileManager, SettingsManager, ThemeManager,
};

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

#[derive(Clone)]
struct AppState {
    profile_manager: Arc<ProfileManager>,
    settings_manager: Arc<SettingsManager>,
    ffmpeg_handler: Arc<FFmpegHandler>,
    ffmpeg_downloader: Arc<AsyncMutex<FFmpegDownloader>>,
    theme_manager: Arc<ThemeManager>,
    obs_handler: Arc<ObsWebSocketHandler>,
    discord_service: Arc<DiscordWebhookService>,
    chat_manager: Arc<ChatManager>,
    oauth_service: Arc<OAuthService>,
    event_bus: EventBus,
    log_dir: PathBuf,
    app_data_dir: PathBuf,
    auth_token: Option<String>,
    rate_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
    // Allowed export directories for path validation
    home_dir: Option<PathBuf>,
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
    fn new(log_dir: &std::path::Path, event_bus: EventBus) -> Result<Self, Box<dyn std::error::Error>> {
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
        let date = timestamp.format("%Y-%m-%d");
        let time = timestamp.format("%H:%M:%S");
        let target = record.target();
        let level = record.level();
        let message = format!("{}", record.args());
        let line = format!("[{date}][{time}][{target}][{level}] {message}");

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

/// Constant-time token comparison to prevent timing attacks
fn verify_token(expected: &str, provided: &str) -> bool {
    expected.as_bytes().ct_eq(provided.as_bytes()).into()
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

/// Mask sensitive patterns in text (tokens, keys, passwords, ENC:: values)
fn mask_sensitive(text: &str) -> String {
    use regex::Regex;
    use std::sync::OnceLock;

    static TOKEN_RE: OnceLock<Regex> = OnceLock::new();
    static ENC_RE: OnceLock<Regex> = OnceLock::new();

    // Match long alphanumeric strings that follow keywords like token, key, password, secret, Bearer
    let token_re = TOKEN_RE.get_or_init(|| {
        Regex::new(r#"(?i)(token|key|password|secret|bearer|oauth|access_token|refresh_token|authorization)[=:\s]+['"]?([A-Za-z0-9_\-./+]{20,})['"]?"#).unwrap()
    });
    // Match ENC:: prefixed values
    let enc_re = ENC_RE.get_or_init(|| {
        Regex::new(r#"ENC::[A-Za-z0-9+/=]{10,}"#).unwrap()
    });

    let result = token_re.replace_all(text, "$1=[REDACTED]");
    enc_re.replace_all(&result, "[ENCRYPTED]").to_string()
}

/// Redact sensitive keys from a JSON payload before logging
fn redact_payload(value: &Value) -> Value {
    const REDACT_KEYS: &[&str] = &[
        "token", "key", "password", "secret", "oauth", "accessToken",
        "refreshToken", "oauthToken", "apiKey", "access_token", "refresh_token",
        "session_token", "webhookUrl",
    ];

    match value {
        Value::Object(map) => {
            let mut redacted = serde_json::Map::new();
            for (k, v) in map {
                let lower = k.to_lowercase();
                if REDACT_KEYS.iter().any(|s| lower.contains(&s.to_lowercase())) {
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

/// Sanitize error messages to prevent information disclosure
fn sanitize_error(error: &str) -> String {
    let masked = mask_sensitive(error);
    log::warn!("[sanitize_error] Original error: {}", masked);
    eprintln!("[sanitize_error] Original error: {}", masked);
    let lower = error.to_lowercase();

    if lower.contains("failed to read") || lower.contains("no such file") || lower.contains("not found") {
        return "Resource not found".to_string();
    }
    // Chat platform errors - pass through user-friendly messages
    if lower.contains("does not exist on twitch") || lower.contains("channel") && lower.contains("not found") {
        return error.to_string();
    }
    if lower.contains("failed to connect to") {
        return error.to_string();
    }
    if lower.contains("no active live broadcast") || lower.contains("not currently live") {
        return error.to_string();
    }
    if lower.contains("already connected") || lower.contains("not connected") {
        return error.to_string();
    }
    if lower.contains("no youtube oauth token") || lower.contains("no twitch oauth token") || lower.contains("please sign in") {
        return error.to_string();
    }
    if lower.contains("parse") || lower.contains("invalid") {
        return "Invalid request format".to_string();
    }
    if lower.contains("permission") || lower.contains("access") || lower.contains("denied") {
        return "Access denied".to_string();
    }
    if lower.contains("traversal") || lower.contains("outside") {
        return "Invalid path".to_string();
    }
    if lower.contains("encrypt") || lower.contains("decrypt") {
        return "Encryption error".to_string();
    }
    // Discord webhook errors - pass through user-friendly messages
    if lower.contains("webhook") || lower.contains("discord") || lower.contains("rate limit") {
        return error.to_string();
    }
    // Network errors - safe to show
    if lower.contains("request failed") || lower.contains("connection") || lower.contains("timeout") {
        return error.to_string();
    }
    // Missing argument errors - safe to show
    if lower.contains("missing argument") {
        return error.to_string();
    }
    // Unknown command errors - safe to show for debugging
    if lower.contains("unknown command") {
        return error.to_string();
    }

    // Return generic message for unknown errors in production
    // In debug mode, we could log the actual error server-side
    log::debug!("Sanitized error: {error}");
    "Operation failed".to_string()
}

// ============================================================================
// Chat Auto-Connect / Auto-Disconnect (tied to stream lifecycle)
// ============================================================================

/// Ensure an OAuth token is fresh, refreshing it via the OAuth service if expired.
/// Returns the (possibly refreshed) access token, and updates settings on disk if refreshed.
/// Adds a 5-minute buffer so we refresh tokens that will expire within the next 5 minutes.
async fn ensure_fresh_oauth_token(
    provider: &str,
    access_token: &str,
    refresh_token: &str,
    expires_at: i64,
    oauth_service: &OAuthService,
    settings_manager: &SettingsManager,
) -> Result<String, String> {
    if access_token.is_empty() {
        return Err(format!("No {} OAuth token available", provider));
    }

    // Check if token is expired or will expire within 5 minutes
    let now = chrono::Utc::now().timestamp();
    let needs_refresh = expires_at > 0 && now >= (expires_at - 300);

    if !needs_refresh {
        return Ok(access_token.to_string());
    }

    if refresh_token.is_empty() {
        return Err(format!("{} token expired and no refresh token available", provider));
    }

    log::info!("{} OAuth token expired (expired {}s ago), refreshing...", provider, now - expires_at);

    let tokens = oauth_service.refresh_token(provider, refresh_token).await?;

    // Update settings with new token
    let mut settings = settings_manager.load()?;
    let new_expires_at = tokens.expires_in.map(|s| now + s as i64).unwrap_or(0);

    match provider {
        "twitch" => {
            settings.twitch_oauth_access_token = tokens.access_token.clone();
            if let Some(ref rt) = tokens.refresh_token {
                settings.twitch_oauth_refresh_token = rt.clone();
            }
            settings.twitch_oauth_expires_at = new_expires_at;
        }
        "youtube" => {
            settings.youtube_oauth_access_token = tokens.access_token.clone();
            if let Some(ref rt) = tokens.refresh_token {
                settings.youtube_oauth_refresh_token = rt.clone();
            }
            settings.youtube_oauth_expires_at = new_expires_at;
        }
        _ => {}
    }

    settings_manager.save(&settings)?;
    log::info!("{} OAuth token refreshed successfully (new expiry in {}s)", provider,
        tokens.expires_in.unwrap_or(0));

    Ok(tokens.access_token)
}

fn build_hour_keys(start: DateTime<Local>, end: DateTime<Local>) -> Vec<String> {
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
        cursor = cursor + Duration::hours(1);
    }

    keys
}

/// Auto-connect all configured chat platforms when a stream starts.
/// Runs as a fire-and-forget background task -- errors are logged, never block the stream.
async fn auto_connect_chat_platforms(
    chat_manager: Arc<ChatManager>,
    settings_manager: Arc<SettingsManager>,
    oauth_service: Arc<OAuthService>,
    event_bus: EventBus,
    ffmpeg_handler: Arc<FFmpegHandler>,
) {
    let settings = match settings_manager.load() {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to load settings for chat auto-connect: {e}");
            return;
        }
    };

    // Twitch: refresh token if needed, then connect immediately (IRC works even when offline)
    if !settings.chat_twitch_channel.is_empty() {
        let already_connected = chat_manager
            .get_platform_status(ChatPlatform::Twitch)
            .await
            .map(|s| s.status == spiritstream_server::models::ChatConnectionStatus::Connected)
            .unwrap_or(false);

        if !already_connected {
            if !settings.twitch_oauth_access_token.is_empty() {
                // Refresh token if expired
                match ensure_fresh_oauth_token(
                    "twitch",
                    &settings.twitch_oauth_access_token,
                    &settings.twitch_oauth_refresh_token,
                    settings.twitch_oauth_expires_at,
                    &oauth_service,
                    &settings_manager,
                )
                .await
                {
                    Ok(fresh_token) => {
                        let mut fresh_settings = settings.clone();
                        fresh_settings.twitch_oauth_access_token = fresh_token;
                        connect_twitch_chat(&chat_manager, &fresh_settings, &event_bus).await;
                    }
                    Err(e) => {
                        log::warn!("Twitch token refresh failed, trying with existing token: {e}");
                        connect_twitch_chat(&chat_manager, &settings, &event_bus).await;
                    }
                }
            } else {
                connect_twitch_chat(&chat_manager, &settings, &event_bus).await;
            }
        } else {
            log::debug!("Twitch chat already connected, skipping auto-connect");
        }
    }

    // YouTube: connect with retry (broadcast won't be live until OBS starts streaming)
    if !settings.chat_youtube_channel_id.is_empty() {
        let has_oauth = !settings.youtube_oauth_access_token.is_empty();
        let has_api_key = !settings.chat_youtube_api_key.is_empty();

        if has_oauth || has_api_key {
            let already_connected = chat_manager
                .get_platform_status(ChatPlatform::YouTube)
                .await
                .map(|s| s.status == spiritstream_server::models::ChatConnectionStatus::Connected)
                .unwrap_or(false);

            if !already_connected {
                // Spawn as separate task -- retries can take up to 5 minutes
                tokio::spawn(connect_youtube_chat_with_retry(
                    chat_manager.clone(),
                    settings_manager.clone(),
                    oauth_service.clone(),
                    event_bus.clone(),
                    ffmpeg_handler,
                ));
            } else {
                log::debug!("YouTube chat already connected, skipping auto-connect");
            }
        }
    }
}

async fn connect_twitch_chat(
    chat_manager: &Arc<ChatManager>,
    settings: &Settings,
    event_bus: &EventBus,
) {
    let auth = if settings.twitch_oauth_access_token.is_empty() {
        None
    } else {
        Some(TwitchAuth::AppOAuth {
            access_token: settings.twitch_oauth_access_token.clone(),
            refresh_token: Some(settings.twitch_oauth_refresh_token.clone())
                .filter(|s| !s.is_empty()),
            expires_at: if settings.twitch_oauth_expires_at > 0 {
                Some(settings.twitch_oauth_expires_at)
            } else {
                None
            },
        })
    };

    let config = ChatConfig {
        platform: ChatPlatform::Twitch,
        enabled: true,
        credentials: ChatCredentials::Twitch {
            channel: settings.chat_twitch_channel.clone(),
            auth,
        },
    };
    match chat_manager.connect(config).await {
        Ok(()) => {
            log::info!("Auto-connected to Twitch chat");
            event_bus.emit("chat_auto_connected", json!({ "platform": "twitch" }));
        }
        Err(e) => {
            if e.to_lowercase().contains("already connected") {
                log::debug!("Twitch chat already connected");
            } else {
                log::warn!("Failed to auto-connect Twitch chat: {e}");
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
async fn connect_youtube_chat_with_retry(
    chat_manager: Arc<ChatManager>,
    settings_manager: Arc<SettingsManager>,
    oauth_service: Arc<OAuthService>,
    event_bus: EventBus,
    ffmpeg_handler: Arc<FFmpegHandler>,
) {
    let build_config = |s: &Settings, token_override: Option<&str>| -> ChatConfig {
        let auth = if !s.youtube_oauth_access_token.is_empty() || token_override.is_some() {
            YouTubeAuth::AppOAuth {
                access_token: token_override
                    .unwrap_or(&s.youtube_oauth_access_token)
                    .to_string(),
                refresh_token: Some(s.youtube_oauth_refresh_token.clone())
                    .filter(|t| !t.is_empty()),
                expires_at: if s.youtube_oauth_expires_at > 0 {
                    Some(s.youtube_oauth_expires_at)
                } else {
                    None
                },
            }
        } else {
            YouTubeAuth::ApiKey {
                key: s.chat_youtube_api_key.clone(),
            }
        };
        ChatConfig {
            platform: ChatPlatform::YouTube,
            enabled: true,
            credentials: ChatCredentials::YouTube {
                channel_id: s.chat_youtube_channel_id.clone(),
                auth,
            },
        }
    };

    // Phase 1: Wait for stream_stats (OBS connected, data flowing)
    log::info!("YouTube chat: waiting for stream data before connecting...");
    let mut rx = event_bus.subscribe();
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
    if ffmpeg_handler.active_count() == 0 {
        log::info!("YouTube chat: stream stopped before OBS data arrived");
        return;
    }

    // Phase 2: Data is flowing. Give YouTube ~10s to register the broadcast.
    log::info!("Stream data detected -- waiting 10s for YouTube to register broadcast...");
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    // Phase 3: Attempt connect with a few retries (15s apart)
    // Load fresh settings each attempt so we pick up refreshed tokens
    const MAX_RETRIES: u32 = 6; // 6 x 15s = 90s of retries after initial wait
    for attempt in 0..=MAX_RETRIES {
        if ffmpeg_handler.active_count() == 0 {
            log::info!("YouTube chat: stream stopped, cancelling connect");
            return;
        }

        // Load fresh settings and refresh token if needed
        let settings = match settings_manager.load() {
            Ok(s) => s,
            Err(e) => {
                log::warn!("YouTube chat: failed to load settings: {e}");
                return;
            }
        };

        // Refresh YouTube OAuth token if expired (skip for API key mode)
        let fresh_token = if !settings.youtube_oauth_access_token.is_empty() {
            match ensure_fresh_oauth_token(
                "youtube",
                &settings.youtube_oauth_access_token,
                &settings.youtube_oauth_refresh_token,
                settings.youtube_oauth_expires_at,
                &oauth_service,
                &settings_manager,
            ).await {
                Ok(token) => Some(token),
                Err(e) => {
                    log::warn!("YouTube token refresh failed: {e}");
                    None // try with existing token anyway
                }
            }
        } else {
            None
        };

        let config = build_config(&settings, fresh_token.as_deref());
        match chat_manager.connect(config).await {
            Ok(()) => {
                log::info!("Auto-connected to YouTube chat");
                event_bus.emit("chat_auto_connected", json!({ "platform": "youtube" }));
                return;
            }
            Err(e) => {
                let lower = e.to_lowercase();
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
                    }
                } else {
                    log::warn!("Failed to auto-connect YouTube chat: {e}");
                    return;
                }
            }
        }
    }
}

/// Auto-disconnect all chat platforms when all streams stop.
async fn auto_disconnect_chat_platforms(
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
async fn start_youtube_token_refresh_task(
    chat_manager: Arc<ChatManager>,
    settings_manager: Arc<SettingsManager>,
    oauth_service: Arc<OAuthService>,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            interval.tick().await;

            let is_connected = chat_manager
                .get_platform_status(ChatPlatform::YouTube)
                .await
                .map(|s| s.status == spiritstream_server::models::ChatConnectionStatus::Connected)
                .unwrap_or(false);

            if !is_connected {
                continue;
            }

            let settings = match settings_manager.load() {
                Ok(s) => s,
                Err(e) => {
                    log::warn!("YouTube token refresh: failed to load settings: {e}");
                    continue;
                }
            };

            if settings.youtube_oauth_access_token.is_empty()
                || settings.youtube_oauth_refresh_token.is_empty()
                || settings.youtube_oauth_expires_at <= 0
            {
                continue;
            }

            let previous_token = settings.youtube_oauth_access_token.clone();
            match ensure_fresh_oauth_token(
                "youtube",
                &previous_token,
                &settings.youtube_oauth_refresh_token,
                settings.youtube_oauth_expires_at,
                &oauth_service,
                &settings_manager,
            )
            .await
            {
                Ok(fresh_token) => {
                    if fresh_token != previous_token {
                        if let Err(e) = chat_manager
                            .update_platform_token(ChatPlatform::YouTube, fresh_token)
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
async fn start_chat_reconnect_task(
    chat_manager: Arc<ChatManager>,
    settings_manager: Arc<SettingsManager>,
    oauth_service: Arc<OAuthService>,
    event_bus: EventBus,
    ffmpeg_handler: Arc<FFmpegHandler>,
) {
    tokio::spawn(async move {
        let mut last_attempts: HashMap<ChatPlatform, Instant> = HashMap::new();
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            interval.tick().await;

            if ffmpeg_handler.active_count() == 0 {
                continue;
            }

            let statuses = chat_manager.get_status().await;
            for status in statuses {
                if status.status != spiritstream_server::models::ChatConnectionStatus::Error {
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

                let settings = match settings_manager.load() {
                    Ok(s) => s,
                    Err(e) => {
                        log::warn!("Chat reconnect: failed to load settings: {e}");
                        continue;
                    }
                };

                match status.platform {
                    ChatPlatform::Twitch => {
                        if settings.chat_twitch_channel.is_empty() {
                            continue;
                        }
                        connect_twitch_chat(&chat_manager, &settings, &event_bus).await;
                    }
                    ChatPlatform::YouTube => {
                        if settings.chat_youtube_channel_id.is_empty()
                            || (settings.youtube_oauth_access_token.is_empty()
                                && settings.chat_youtube_api_key.is_empty())
                        {
                            continue;
                        }
                        tokio::spawn(connect_youtube_chat_with_retry(
                            chat_manager.clone(),
                            settings_manager.clone(),
                            oauth_service.clone(),
                            event_bus.clone(),
                            ffmpeg_handler.clone(),
                        ));
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

// ============================================================================
// CORS Configuration
// ============================================================================

fn build_cors_layer() -> CorsLayer {
    let cors_origins = env::var("SPIRITSTREAM_CORS_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:*,http://127.0.0.1:*,tauri://localhost,http://tauri.localhost,https://tauri.localhost".to_string());

    let allowed_origins: Vec<String> = cors_origins
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(move |origin: &HeaderValue, _| {
            let origin_str = match origin.to_str() {
                Ok(s) => s,
                Err(_) => return false,
            };

            allowed_origins.iter().any(|allowed| {
                if allowed.ends_with(":*") {
                    // Wildcard port matching
                    let prefix = allowed.trim_end_matches(":*");
                    origin_str.starts_with(prefix) && origin_str[prefix.len()..].starts_with(':')
                } else {
                    origin_str == allowed
                }
            })
        }))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::COOKIE, header::AUTHORIZATION])
        .allow_credentials(true)
}

// ============================================================================
// Authentication Endpoints
// ============================================================================

#[derive(Deserialize)]
struct LoginRequest {
    token: String,
}

/// Set a session cookie
fn set_session_cookie(cookies: &Cookies) {
    let session_id = uuid::Uuid::new_v4().to_string();
    let cookie = Cookie::build((AUTH_COOKIE_NAME, session_id))
        .http_only(true)
        .secure(false) // Set to true when using HTTPS
        .same_site(tower_cookies::cookie::SameSite::Strict)
        .path("/")
        .max_age(tower_cookies::cookie::time::Duration::seconds(COOKIE_MAX_AGE_SECS))
        .build();
    cookies.add(cookie);
}

/// POST /auth/login - Validate token and set HttpOnly cookie
async fn auth_login(
    State(state): State<AppState>,
    cookies: Cookies,
    Json(payload): Json<LoginRequest>,
) -> impl IntoResponse {
    let expected_token = state.auth_token.as_deref();

    match expected_token {
        None => {
            // No token configured - open access, set session cookie anyway
            set_session_cookie(&cookies);
            Json(json!({ "ok": true }))
        }
        Some(expected) if verify_token(expected, &payload.token) => {
            set_session_cookie(&cookies);
            Json(json!({ "ok": true }))
        }
        _ => {
            // Invalid token - add a small delay to prevent brute force
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            Json(json!({ "ok": false, "error": "Invalid token" }))
        }
    }
}

/// POST /auth/logout - Clear session cookie
async fn auth_logout(cookies: Cookies) -> impl IntoResponse {
    let cookie = Cookie::build((AUTH_COOKIE_NAME, ""))
        .path("/")
        .max_age(tower_cookies::cookie::time::Duration::ZERO)
        .build();
    cookies.remove(cookie);
    Json(json!({ "ok": true }))
}

/// GET /auth/check - Check if session is valid
async fn auth_check(
    State(state): State<AppState>,
    cookies: Cookies,
) -> impl IntoResponse {
    // If no token configured, always authenticated
    if state.auth_token.is_none() {
        return Json(json!({ "authenticated": true, "required": false }));
    }

    let is_authenticated = cookies.get(AUTH_COOKIE_NAME).is_some();
    Json(json!({ "authenticated": is_authenticated, "required": true }))
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

    // Check for valid session cookie
    if cookies.get(AUTH_COOKIE_NAME).is_some() {
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

/// Rate limiting middleware
async fn rate_limit_middleware(
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    match state.rate_limiter.check() {
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

// ============================================================================
// Request Handlers
// ============================================================================

async fn health() -> impl IntoResponse {
    Json(json!({ "ok": true }))
}

/// Readiness check - verifies critical services are functional
    async fn ready(State(state): State<AppState>) -> impl IntoResponse {
        #[derive(Debug, Serialize)]
        struct ReadyCheckError {
            check: &'static str,
            error: String,
        }

        let mut errors: Vec<ReadyCheckError> = Vec::new();

        // Check 1: ProfileManager can access profiles directory
        let profiles_ok = match state.profile_manager.get_all_names().await {
            Ok(_) => true,
            Err(err) => {
                errors.push(ReadyCheckError {
                    check: "profiles",
                    error: err,
                });
                false
            }
        };

        // Check 2: SettingsManager can load settings
        let settings_ok = match state.settings_manager.load() {
            Ok(_) => true,
            Err(err) => {
                errors.push(ReadyCheckError {
                    check: "settings",
                    error: err,
                });
                false
            }
        };

        // Check 3: ThemeManager initialized (theme list is always available after init)
        let themes_ok = true;

        let all_ok = profiles_ok && settings_ok && themes_ok;
        let failed: Vec<&str> = errors.iter().map(|item| item.check).collect();

        if all_ok {
            Json(json!({ "ready": true })).into_response()
        } else {
            log::warn!("Readiness check failed: {errors:?}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "ready": false,
                    "failed": failed,
                    "errors": errors,
                })),
            )
                .into_response()
        }
    }

// ============================================================================
// File Browser Endpoints (for HTTP mode dialogs)
// ============================================================================

#[derive(Debug, Deserialize)]
struct FileBrowseQuery {
    path: Option<String>,
}

#[derive(Debug, Serialize)]
struct FileEntry {
    name: String,
    #[serde(rename = "type")]
    entry_type: String, // "file" or "directory"
    size: Option<u64>,
}

#[derive(Debug, Serialize)]
struct BrowseResponse {
    path: String,
    entries: Vec<FileEntry>,
    parent: Option<String>,
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
) -> impl IntoResponse {
    // Determine the directory to browse
    let browse_path = match params.path {
        Some(p) if !p.is_empty() => PathBuf::from(&p),
        _ => match &state.home_dir {
            Some(home) => home.clone(),
            None => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "ok": false, "error": "Cannot determine home directory" })),
                )
                    .into_response();
            }
        },
    };

    // Security: Validate path is within allowed directories
    // Include common binary directories for finding executables like FFmpeg
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

    if validate_path_within_any(&browse_path, &allowed_dirs).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "ok": false, "error": "Access to this directory is not allowed" })),
        )
            .into_response();
    }

    // Check if path exists and is a directory
    if !browse_path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "ok": false, "error": "Directory not found" })),
        )
            .into_response();
    }

    if !browse_path.is_dir() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": "Path is not a directory" })),
        )
            .into_response();
    }

    // Read directory entries
    let entries = match std::fs::read_dir(&browse_path) {
        Ok(entries) => entries,
        Err(e) => {
            log::error!("Failed to read directory {browse_path:?}: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": "Failed to read directory" })),
            )
                .into_response();
        }
    };

    let mut file_entries: Vec<FileEntry> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files/directories (starting with .)
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

    // Sort: directories first, then alphabetically
    file_entries.sort_by(|a, b| {
        match (&a.entry_type[..], &b.entry_type[..]) {
            ("directory", "file") => std::cmp::Ordering::Less,
            ("file", "directory") => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    // Calculate parent directory (if not at root)
    let parent = browse_path.parent().and_then(|p| {
        let parent_path = p.to_path_buf();
        // Only include parent if it's within allowed directories
        if validate_path_within_any(&parent_path, &allowed_dirs).is_ok() {
            Some(parent_path.to_string_lossy().to_string())
        } else {
            None
        }
    });

    let response = BrowseResponse {
        path: browse_path.to_string_lossy().to_string(),
        entries: file_entries,
        parent,
    };

    Json(json!({ "ok": true, "data": response })).into_response()
}

/// GET /api/files/home - Get user home directory path
async fn files_home(State(state): State<AppState>) -> impl IntoResponse {
    match &state.home_dir {
        Some(home) => Json(json!({
            "ok": true,
            "data": { "path": home.to_string_lossy().to_string() }
        })),
        None => Json(json!({
            "ok": false,
            "error": "Cannot determine home directory"
        })),
    }
}

#[derive(Debug, Deserialize)]
struct OpenPathRequest {
    path: String,
}

/// POST /api/files/open - Open path in native file manager
async fn files_open(
    State(state): State<AppState>,
    Json(payload): Json<OpenPathRequest>,
) -> impl IntoResponse {
    let path = PathBuf::from(&payload.path);

    // Security: Validate path is within allowed directories
    // Include common binary directories for consistency with file browser
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

    if validate_path_within_any(&path, &allowed_dirs).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "ok": false, "error": "Access to this path is not allowed" })),
        );
    }

    // Check if path exists
    if !path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "ok": false, "error": "Path not found" })),
        );
    }

    // Open in native file manager
    match opener::open(&path) {
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))),
        Err(e) => {
            log::error!("Failed to open path {path:?}: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": "Failed to open path" })),
            )
        }
    }
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
            state.auth_token.as_deref().is_some_and(|expected| verify_token(expected, token))
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

fn unauthorized_response() -> impl IntoResponse {
    let response = InvokeResponse {
        ok: false,
        data: None,
        error: Some("Unauthorized".to_string()),
    };
    (StatusCode::UNAUTHORIZED, Json(response))
}

async fn invoke(
    Path(command): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    // Authentication check (cookie or bearer token)
    if let Some(expected) = state.auth_token.as_deref() {
        let authenticated = cookies.get(AUTH_COOKIE_NAME).is_some()
            || bearer_token(&headers).is_some_and(|t| verify_token(expected, t));

        if !authenticated {
            return unauthorized_response().into_response();
        }
    }

    let result = invoke_command(&state, &command, payload).await;

    match result {
        Ok(data) => {
            let response = InvokeResponse {
                ok: true,
                data: Some(data),
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(error) => {
            let response = InvokeResponse {
                ok: false,
                data: None,
                error: Some(sanitize_error(&error)),
            };
            (StatusCode::BAD_REQUEST, Json(response)).into_response()
        }
    }
}

// ============================================================================
// Command Handler
// ============================================================================

/// Commands that are called frequently for polling and don't need logging
const QUIET_COMMANDS: &[&str] = &[
    "get_chat_status",
    "get_platform_chat_status",
    "is_chat_connected",
    "obs_get_state",
    "obs_is_connected",
    "get_active_stream_count",
    "get_active_group_ids",
];

async fn invoke_command(
    state: &AppState,
    command: &str,
    payload: Value,
) -> Result<Value, String> {
    // Only log non-polling commands to reduce noise
    if !QUIET_COMMANDS.contains(&command) {
        let safe_payload = redact_payload(&payload);
        log::info!("[invoke_command] Command: {}, Payload: {:?}", command, safe_payload);
    }

    let result = match command {
        "get_all_profiles" => {
            let names = state.profile_manager.get_all_names().await?;
            Ok(json!(names))
        }
        "get_profile_summaries" => {
            let summaries = state.profile_manager.get_all_summaries().await?;
            Ok(json!(summaries))
        }
        "load_profile" => {
            let name: String = get_arg(&payload, "name")?;
            let password: Option<String> = get_opt_arg(&payload, "password")?;
            let profile = state
                .profile_manager
                .load_with_key_decryption(&name, password.as_deref())
                .await?;
            Ok(json!(profile))
        }
        "save_profile" => {
            let profile: Profile = get_arg(&payload, "profile")?;
            let password: Option<String> = get_opt_arg(&payload, "password")?;
            let settings = state.settings_manager.load()?;
            state
                .profile_manager
                .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
                .await?;
            state.event_bus.emit("profile_changed", json!({ "action": "saved", "name": profile.name }));
            Ok(Value::Null)
        }
        "delete_profile" => {
            let name: String = get_arg(&payload, "name")?;
            state.profile_manager.delete(&name).await?;
            state.event_bus.emit("profile_changed", json!({ "action": "deleted", "name": name }));
            Ok(Value::Null)
        }
        "is_profile_encrypted" => {
            let name: String = get_arg(&payload, "name")?;
            Ok(json!(state.profile_manager.is_encrypted(&name)))
        }
        "validate_input" => {
            let profile_id: String = get_arg(&payload, "profileId")?;
            let input: RtmpInput = get_arg(&payload, "input")?;
            state.profile_manager.validate_input_conflict(&profile_id, &input).await?;
            Ok(Value::Null)
        }
        "set_profile_order" => {
            let ordered_names: Vec<String> = get_arg(&payload, "orderedNames")?;
            let mut map = state.profile_manager.read_order_index_map()?;
            let existing = state.profile_manager.get_all_names().await?;

            let mut idx = 0;
            for name in ordered_names {
                if !existing.contains(&name) {
                    return Err(format!("Unknown profile: {name}"));
                }
                idx += 10;
                map.insert(name, idx);
            }

            state.profile_manager.write_order_index_map(&map)?;
            Ok(Value::Null)
        }
        "get_order_index_map" => {
            let map = state.profile_manager.read_order_index_map()?;
            Ok(json!(map))
        }
        "ensure_order_indexes" => {
            let map = state.profile_manager.ensure_order_indexes().await?;
            Ok(json!(map))
        }
        "start_stream" => {
            let group: OutputGroup = get_arg(&payload, "group")?;
            let incoming_url: String = get_arg(&payload, "incomingUrl")?;
            let was_streaming = state.ffmpeg_handler.active_count() > 0;
            let event_sink: Arc<dyn EventSink> = Arc::new(state.event_bus.clone());
            let pid = state.ffmpeg_handler.start(&group, &incoming_url, event_sink)?;
            // Reset reconnection state on successful manual start
            state.ffmpeg_handler.reset_reconnection_state(&group.id);
            // Auto-connect chat platforms on first stream start
            if !was_streaming {
                state.chat_manager.start_log_session();
                let chat_mgr = state.chat_manager.clone();
                let settings_mgr = state.settings_manager.clone();
                let oauth_svc = state.oauth_service.clone();
                let bus = state.event_bus.clone();
                let ffmpeg = state.ffmpeg_handler.clone();
                tokio::spawn(auto_connect_chat_platforms(chat_mgr, settings_mgr, oauth_svc, bus, ffmpeg));
            }
            Ok(json!(pid))
        }
        "start_all_streams" => {
            let groups: Vec<OutputGroup> = get_arg(&payload, "groups")?;
            let incoming_url: String = get_arg(&payload, "incomingUrl")?;
            let was_streaming = state.ffmpeg_handler.active_count() > 0;
            let event_sink: Arc<dyn EventSink> = Arc::new(state.event_bus.clone());
            let pids = state.ffmpeg_handler.start_all(&groups, &incoming_url, event_sink)?;
            // Auto-connect chat platforms when streams start
            if !was_streaming {
                state.chat_manager.start_log_session();
            }
            let chat_mgr = state.chat_manager.clone();
            let settings_mgr = state.settings_manager.clone();
            let oauth_svc = state.oauth_service.clone();
            let bus = state.event_bus.clone();
            let ffmpeg = state.ffmpeg_handler.clone();
            tokio::spawn(auto_connect_chat_platforms(chat_mgr, settings_mgr, oauth_svc, bus, ffmpeg));
            Ok(json!(pids))
        }
        "stop_stream" => {
            let group_id: String = get_arg(&payload, "groupId")?;
            state.ffmpeg_handler.stop(&group_id)?;
            // Auto-disconnect chat when no more streams are running
            if state.ffmpeg_handler.active_count() == 0 {
                state.chat_manager.end_log_session();
                let chat_mgr = state.chat_manager.clone();
                let bus = state.event_bus.clone();
                tokio::spawn(auto_disconnect_chat_platforms(chat_mgr, bus));
            }
            Ok(Value::Null)
        }
        "stop_all_streams" => {
            state.ffmpeg_handler.stop_all()?;
            state.chat_manager.end_log_session();
            // Auto-disconnect all chat platforms
            let chat_mgr = state.chat_manager.clone();
            let bus = state.event_bus.clone();
            tokio::spawn(auto_disconnect_chat_platforms(chat_mgr, bus));
            Ok(Value::Null)
        }
        "retry_stream" => {
            let group_id: String = get_arg(&payload, "groupId")?;
            let event_sink: Arc<dyn EventSink> = Arc::new(state.event_bus.clone());
            let ffmpeg_handler = state.ffmpeg_handler.clone();
            // Use spawn_blocking to avoid blocking the async runtime during backoff sleep
            let (pid, next_delay) = tokio::task::spawn_blocking(move || {
                ffmpeg_handler.retry_group(&group_id, event_sink)
            })
            .await
            .map_err(|e| format!("Task join error: {e}"))??;
            Ok(json!({
                "pid": pid,
                "nextDelaySecs": next_delay.map(|d| d.as_secs())
            }))
        }
        "get_active_stream_count" => Ok(json!(state.ffmpeg_handler.active_count())),
        "is_group_streaming" => {
            let group_id: String = get_arg(&payload, "groupId")?;
            Ok(json!(state.ffmpeg_handler.is_streaming(&group_id)))
        }
        "get_active_group_ids" => Ok(json!(state.ffmpeg_handler.get_active_group_ids())),
        "toggle_stream_target" => {
            let target_id: String = get_arg(&payload, "targetId")?;
            let enabled: bool = get_arg(&payload, "enabled")?;
            let group: OutputGroup = get_arg(&payload, "group")?;
            let incoming_url: String = get_arg(&payload, "incomingUrl")?;
            if enabled {
                state.ffmpeg_handler.enable_target(&target_id);
            } else {
                state.ffmpeg_handler.disable_target(&target_id);
            }
            let event_sink: Arc<dyn EventSink> = Arc::new(state.event_bus.clone());
            let pid = state
                .ffmpeg_handler
                .restart_group(&group.id, &group, &incoming_url, event_sink)?;
            Ok(json!(pid))
        }
        "is_target_disabled" => {
            let target_id: String = get_arg(&payload, "targetId")?;
            Ok(json!(state.ffmpeg_handler.is_target_disabled(&target_id)))
        }
        "get_encoders" => Ok(json!(get_encoders()?)),
        "test_ffmpeg" => Ok(json!(test_ffmpeg()?)),
        "validate_ffmpeg_path" => {
            let path: String = get_arg(&payload, "path")?;
            Ok(json!(validate_ffmpeg_path(path)?))
        }
        "test_rtmp_target" => {
            let url: String = get_arg(&payload, "url")?;
            let stream_key: String = get_arg(&payload, "streamKey")?;
            Ok(json!(test_rtmp_target(url, stream_key)?))
        }
        "get_recent_logs" => {
            let max_lines: Option<usize> = get_opt_arg(&payload, "maxLines")?;
            Ok(json!(read_recent_logs(&state.log_dir, max_lines.unwrap_or(500))?))
        }
        "export_logs" => {
            let path: String = get_arg(&payload, "path")?;
            let content: String = get_arg(&payload, "content")?;

            // Security: Validate export path is within allowed directories
            let export_path = PathBuf::from(&path);

            // Build list of allowed directories
            let mut allowed_dirs: Vec<&std::path::Path> = vec![state.app_data_dir.as_path()];
            if let Some(ref home) = state.home_dir {
                allowed_dirs.push(home.as_path());
            }

            // Validate the path
            validate_path_within_any(&export_path, &allowed_dirs)?;

            std::fs::write(&path, content).map_err(|e| format!("Failed to write log file: {e}"))?;
            Ok(Value::Null)
        }
        "get_settings" => Ok(json!(state.settings_manager.load()?)),
        "save_settings" => {
            let settings: Settings = get_arg(&payload, "settings")?;
            state.settings_manager.save(&settings)?;
            let _ = prune_logs(&state.log_dir, settings.log_retention_days);
            state.chat_manager.set_crosspost_enabled(settings.chat_crosspost_enabled);
            state.chat_manager.set_send_enabled(ChatPlatform::Twitch, settings.chat_twitch_send_enabled).await;
            state.chat_manager.set_send_enabled(ChatPlatform::YouTube, settings.chat_youtube_send_enabled).await;
            state.event_bus.emit("settings_changed", json!({}));
            Ok(Value::Null)
        }
        "get_profiles_path" => {
            let path = state.settings_manager.get_profiles_path();
            Ok(json!(path.to_string_lossy().to_string()))
        }
        "export_data" => {
            let export_path: String = get_arg(&payload, "exportPath")?;
            let path = PathBuf::from(&export_path);

            // Security: Validate export path is within allowed directories
            let mut allowed_dirs: Vec<&std::path::Path> = vec![state.app_data_dir.as_path()];
            if let Some(ref home) = state.home_dir {
                allowed_dirs.push(home.as_path());
            }

            validate_path_within_any(&path, &allowed_dirs)?;

            state.settings_manager.export_data(&path)?;
            Ok(Value::Null)
        }
        "clear_data" => {
            state.settings_manager.clear_data()?;
            Ok(Value::Null)
        }
        "rotate_machine_key" => {
            let profiles_dir = state.app_data_dir.join("profiles");
            let report = Encryption::rotate_machine_key(&state.app_data_dir, &profiles_dir)?;
            Ok(json!(report))
        }
        "download_ffmpeg" => {
            let downloader = state.ffmpeg_downloader.lock().await;
            let path = downloader
                .download(&state.event_bus)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!(path.to_string_lossy().to_string()))
        }
        "cancel_ffmpeg_download" => {
            let downloader = state.ffmpeg_downloader.lock().await;
            downloader.cancel();
            Ok(Value::Null)
        }
        "get_bundled_ffmpeg_path" => {
            let path = FFmpegDownloader::get_ffmpeg_path(Some(&state.settings_manager));
            Ok(json!(path.map(|p| p.to_string_lossy().to_string())))
        }
        "check_ffmpeg_update" => {
            let installed_version: Option<String> = get_opt_arg(&payload, "installedVersion")?;
            let downloader = state.ffmpeg_downloader.lock().await;
            let info = downloader
                .check_version_status(installed_version.as_deref())
                .await;
            Ok(json!(info))
        }
        "delete_ffmpeg" => {
            FFmpegDownloader::delete_ffmpeg(Some(&state.settings_manager))
                .map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "list_themes" => {
            let themes = state.theme_manager.list_themes();
            log::info!(
                "[THEME CMD] list_themes returning {} themes: {:?}",
                themes.len(),
                themes.iter().map(|t| &t.id).collect::<Vec<_>>()
            );
            Ok(json!(themes))
        }
        "refresh_themes" => {
            log::info!("[THEME CMD] refresh_themes called");
            state.theme_manager.sync_project_themes();
            let themes = state.theme_manager.list_themes();
            log::info!(
                "[THEME CMD] refresh_themes returning {} themes",
                themes.len()
            );
            Ok(json!(themes))
        }
        "get_theme_tokens" => {
            let theme_id: String = get_arg(&payload, "themeId")?;
            log::info!("[THEME CMD] get_theme_tokens called for: {theme_id}");
            match state.theme_manager.get_theme_tokens(&theme_id) {
                Ok(tokens) => {
                    log::info!(
                        "[THEME CMD] Returning {} tokens for {}",
                        tokens.len(),
                        theme_id
                    );
                    // Log sample tokens for verification
                    for (i, (key, value)) in tokens.iter().take(3).enumerate() {
                        log::info!("[THEME CMD] Sample token {}: {} = {}", i + 1, key, value);
                    }
                    Ok(json!(tokens))
                }
                Err(e) => {
                    log::error!("[THEME CMD] get_theme_tokens failed for {theme_id}: {e}");
                    Err(e)
                }
            }
        }
        "install_theme" => {
            let theme_path: String = get_arg(&payload, "themePath")?;
            let path = PathBuf::from(&theme_path);

            // Security: Validate file extension
            validate_extension(&path, &["json", "jsonc"])?;

            // Security: Reject paths with traversal sequences
            if theme_path.contains("..") {
                return Err("Invalid theme path: path traversal not allowed".to_string());
            }

            Ok(json!(state.theme_manager.install_theme(&path)?))
        }

        // ============================================================================
        // OBS WebSocket Commands
        // ============================================================================
        "obs_get_state" => {
            let obs_state = state.obs_handler.get_state().await;
            Ok(json!(obs_state))
        }
        "obs_get_config" => {
            let config = state.obs_handler.get_config().await;
            let mut response = json!(config);

            // Decrypt the password if it's encrypted
            if let Some(obj) = response.as_object_mut() {
                if let Some(pass_value) = obj.get("password") {
                    let pass_str = pass_value.as_str().unwrap_or("");
                    if !pass_str.is_empty() && Encryption::is_stream_key_encrypted(pass_str) {
                        match Encryption::decrypt_stream_key(pass_str, &state.app_data_dir) {
                            Ok(decrypted) => {
                                obj.insert("password".to_string(), json!(decrypted));
                            }
                            Err(e) => {
                                log::warn!("Failed to decrypt OBS password: {}", e);
                                obj.insert("password".to_string(), json!(""));
                            }
                        }
                    }
                }
            }
            Ok(response)
        }
        "obs_set_config" => {
            let host: String = get_arg(&payload, "host")?;
            let port: u16 = get_arg(&payload, "port")?;
            let password: Option<String> = get_opt_arg(&payload, "password")?;
            let use_auth: bool = get_arg(&payload, "useAuth")?;
            let direction: String = get_arg(&payload, "direction")?;
            let auto_connect: bool = get_arg(&payload, "autoConnect")?;

            // Get current config to preserve existing password if not provided
            let current_config = state.obs_handler.get_config().await;

            // Encrypt password if provided, otherwise keep existing
            let encrypted_password = if let Some(ref pass) = password {
                if pass.is_empty() {
                    String::new()
                } else {
                    state.obs_handler.encrypt_password(pass)?
                }
            } else {
                current_config.password
            };

            // Parse direction
            let dir = match direction.as_str() {
                "obs-to-spiritstream" => spiritstream_server::services::IntegrationDirection::ObsToSpiritstream,
                "spiritstream-to-obs" => spiritstream_server::services::IntegrationDirection::SpiritstreamToObs,
                "bidirectional" => spiritstream_server::services::IntegrationDirection::Bidirectional,
                _ => spiritstream_server::services::IntegrationDirection::Disabled,
            };

            let config = ObsConfig {
                host: host.clone(),
                port,
                password: encrypted_password.clone(),
                use_auth,
                direction: dir,
                auto_connect,
            };

            state.obs_handler.set_config(config).await;

            // Also save to settings
            let mut settings = state.settings_manager.load()?;
            settings.obs_host = host;
            settings.obs_port = port;
            settings.obs_password = encrypted_password;
            settings.obs_use_auth = use_auth;
            settings.obs_direction = match dir {
                spiritstream_server::services::IntegrationDirection::ObsToSpiritstream => ObsIntegrationDirection::ObsToSpiritstream,
                spiritstream_server::services::IntegrationDirection::SpiritstreamToObs => ObsIntegrationDirection::SpiritstreamToObs,
                spiritstream_server::services::IntegrationDirection::Bidirectional => ObsIntegrationDirection::Bidirectional,
                spiritstream_server::services::IntegrationDirection::Disabled => ObsIntegrationDirection::Disabled,
            };
            settings.obs_auto_connect = auto_connect;
            state.settings_manager.save(&settings)?;

            Ok(Value::Null)
        }
        "obs_connect" => {
            state.obs_handler.connect(state.event_bus.clone()).await?;
            Ok(Value::Null)
        }
        "obs_disconnect" => {
            state.obs_handler.disconnect(state.event_bus.clone()).await?;
            Ok(Value::Null)
        }
        "obs_start_stream" => {
            state.obs_handler.start_stream().await?;
            Ok(Value::Null)
        }
        "obs_stop_stream" => {
            state.obs_handler.stop_stream().await?;
            Ok(Value::Null)
        }
        "obs_is_connected" => {
            Ok(json!(state.obs_handler.is_connected().await))
        }

        // ============================================================================
        // Discord Webhook Commands
        // ============================================================================
        "discord_test_webhook" => {
            log::info!("[discord_test_webhook] Received payload: {:?}", payload);
            let url: String = get_arg(&payload, "url")?;
            log::info!("[discord_test_webhook] Testing URL: {}", if url.len() > 50 { &url[..50] } else { &url });
            let result = state.discord_service.test_webhook(&url).await;
            log::info!("[discord_test_webhook] Result: {:?}", result);
            Ok(json!(result))
        }
        "discord_send_notification" => {
            let settings = state.settings_manager.load()?;
            if !settings.discord_webhook_enabled {
                return Ok(json!({
                    "success": false,
                    "message": "Discord webhook is not enabled",
                    "skippedCooldown": false
                }));
            }
            let image_path = if settings.discord_image_path.is_empty() {
                None
            } else {
                Some(settings.discord_image_path.as_str())
            };
            let result = state.discord_service.send_go_live_notification(
                &settings.discord_webhook_url,
                &settings.discord_go_live_message,
                image_path,
                settings.discord_cooldown_enabled,
                settings.discord_cooldown_seconds,
            ).await;
            Ok(json!(result))
        }
        "discord_reset_cooldown" => {
            state.discord_service.reset_cooldown().await;
            Ok(Value::Null)
        }

        // ============================================================================
        // Chat Commands
        // ============================================================================
        "connect_chat" => {
            let mut config: ChatConfig = get_arg(&payload, "config")?;

            // Enrich credentials with stored OAuth tokens when frontend sends empty placeholders,
            // and refresh expired tokens automatically.
            let settings = state.settings_manager.load()?;
            config.credentials = match config.credentials {
                ChatCredentials::Twitch { channel, auth } => {
                    let enriched_auth = match auth {
                        Some(TwitchAuth::AppOAuth { access_token, refresh_token, expires_at })
                            if access_token.is_empty() =>
                        {
                            if settings.twitch_oauth_access_token.is_empty() {
                                return Err("No Twitch OAuth token stored. Please login with Twitch first.".to_string());
                            }
                            // Refresh token if expired
                            let fresh_token = ensure_fresh_oauth_token(
                                "twitch",
                                &settings.twitch_oauth_access_token,
                                &settings.twitch_oauth_refresh_token,
                                settings.twitch_oauth_expires_at,
                                &state.oauth_service,
                                &state.settings_manager,
                            ).await.unwrap_or_else(|e| {
                                log::warn!("Twitch token refresh failed: {e}");
                                settings.twitch_oauth_access_token.clone()
                            });
                            Some(TwitchAuth::AppOAuth {
                                access_token: fresh_token,
                                refresh_token: if refresh_token.is_none() {
                                    Some(settings.twitch_oauth_refresh_token.clone())
                                        .filter(|s| !s.is_empty())
                                } else {
                                    refresh_token
                                },
                                expires_at: if expires_at.is_none() && settings.twitch_oauth_expires_at > 0 {
                                    Some(settings.twitch_oauth_expires_at)
                                } else {
                                    expires_at
                                },
                            })
                        }
                        other => other,
                    };
                    ChatCredentials::Twitch { channel, auth: enriched_auth }
                }
                ChatCredentials::YouTube { channel_id, auth } => {
                    let enriched_auth = match auth {
                        YouTubeAuth::AppOAuth { access_token, refresh_token, expires_at }
                            if access_token.is_empty() =>
                        {
                            if settings.youtube_oauth_access_token.is_empty() {
                                return Err("No YouTube OAuth token stored. Please sign in with Google first.".to_string());
                            }
                            // Refresh token if expired
                            let fresh_token = ensure_fresh_oauth_token(
                                "youtube",
                                &settings.youtube_oauth_access_token,
                                &settings.youtube_oauth_refresh_token,
                                settings.youtube_oauth_expires_at,
                                &state.oauth_service,
                                &state.settings_manager,
                            ).await.unwrap_or_else(|e| {
                                log::warn!("YouTube token refresh failed: {e}");
                                settings.youtube_oauth_access_token.clone()
                            });
                            YouTubeAuth::AppOAuth {
                                access_token: fresh_token,
                                refresh_token: if refresh_token.is_none() {
                                    Some(settings.youtube_oauth_refresh_token.clone())
                                        .filter(|s| !s.is_empty())
                                } else {
                                    refresh_token
                                },
                                expires_at: if expires_at.is_none() && settings.youtube_oauth_expires_at > 0 {
                                    Some(settings.youtube_oauth_expires_at)
                                } else {
                                    expires_at
                                },
                            }
                        }
                        other => other,
                    };
                    ChatCredentials::YouTube { channel_id, auth: enriched_auth }
                }
                other => other,
            };

            state.chat_manager.connect(config).await?;
            Ok(Value::Null)
        }
        "send_chat_message" => {
            let message: String = get_arg(&payload, "message")?;
            let trimmed = message.trim().to_string();
            if trimmed.is_empty() {
                return Err("Message cannot be empty".to_string());
            }

            let settings = state.settings_manager.load()?;
            let mut targets = Vec::new();
            if settings.chat_twitch_send_enabled {
                targets.push(ChatPlatform::Twitch);
            }
            if settings.chat_youtube_send_enabled {
                targets.push(ChatPlatform::YouTube);
            }

            if targets.is_empty() {
                return Err("No chat platforms are enabled for sending".to_string());
            }

            let results = state.chat_manager.send_message(trimmed.clone(), &targets).await;
            let mut send_results: Vec<ChatSendResult> = Vec::new();
            let mut successes: Vec<ChatPlatform> = Vec::new();

            for (platform, result) in results {
                match result {
                    Ok(()) => {
                        successes.push(platform);
                        send_results.push(ChatSendResult {
                            platform,
                            success: true,
                            error: None,
                        });
                    }
                    Err(err) => {
                        send_results.push(ChatSendResult {
                            platform,
                            success: false,
                            error: Some(err),
                        });
                    }
                }
            }

            if !successes.is_empty() {
                let outbound = ChatMessage::new_outbound(
                    successes,
                    "You".to_string(),
                    trimmed,
                );
                state.chat_manager.log_message(outbound.clone());
                if let Ok(payload) = serde_json::to_value(&outbound) {
                    state.event_bus.emit("chat_message", payload);
                }
            }

            Ok(json!(send_results))
        }
        "chat_export_log" => {
            let path: String = get_arg(&payload, "path")?;
            let start_ms = state
                .chat_manager
                .log_session_start_ms()
                .ok_or_else(|| "No active chat session to export".to_string())?;

            // Flush any buffered log lines before exporting
            state.chat_manager.flush_chat_logs().await?;

            let end_ms = chrono::Local::now().timestamp_millis();
            let start_dt = Local
                .timestamp_millis_opt(start_ms)
                .single()
                .unwrap_or_else(Local::now);
            let end_dt = Local
                .timestamp_millis_opt(end_ms)
                .single()
                .unwrap_or_else(Local::now);

            let hour_keys = build_hour_keys(start_dt, end_dt);
            let mut writer = BufWriter::new(
                File::create(&path).map_err(|e| format!("Failed to create export file: {}", e))?,
            );

            for key in hour_keys {
                let src_path = state.log_dir.join(format!("chatlog_{}.jsonl", key));
                if !src_path.exists() {
                    continue;
                }

                let file = File::open(&src_path)
                    .map_err(|e| format!("Failed to read chat log {}: {}", src_path.display(), e))?;
                let reader = BufReader::new(file);
                for line in reader.lines() {
                    let line = line.map_err(|e| format!("Failed to read chat log: {}", e))?;
                    if line.trim().is_empty() {
                        continue;
                    }
                    if let Ok(message) = serde_json::from_str::<ChatMessage>(&line) {
                        if message.timestamp >= start_ms && message.timestamp <= end_ms {
                            writer
                                .write_all(line.as_bytes())
                                .map_err(|e| format!("Failed to write export file: {}", e))?;
                            writer
                                .write_all(b"\n")
                                .map_err(|e| format!("Failed to write export file: {}", e))?;
                        }
                    }
                }
            }

            writer
                .flush()
                .map_err(|e| format!("Failed to finalize export file: {}", e))?;

            Ok(Value::Null)
        }
        "chat_search_session" => {
            let query: String = get_arg(&payload, "query")?;
            let limit: Option<usize> = get_opt_arg(&payload, "limit")?;
            let limit = limit.unwrap_or(500);

            let start_ms = state
                .chat_manager
                .log_session_start_ms()
                .ok_or_else(|| "No active chat session to search".to_string())?;

            let query = query.trim().to_lowercase();
            if query.is_empty() {
                return Ok(json!([]));
            }

            let end_ms = chrono::Local::now().timestamp_millis();
            let start_dt = Local
                .timestamp_millis_opt(start_ms)
                .single()
                .unwrap_or_else(Local::now);
            let end_dt = Local
                .timestamp_millis_opt(end_ms)
                .single()
                .unwrap_or_else(Local::now);

            let hour_keys = build_hour_keys(start_dt, end_dt);
            let mut matches: Vec<ChatMessage> = Vec::new();

            for key in hour_keys {
                if matches.len() >= limit {
                    break;
                }

                let src_path = state.log_dir.join(format!("chatlog_{}.jsonl", key));
                if !src_path.exists() {
                    continue;
                }

                let file = File::open(&src_path)
                    .map_err(|e| format!("Failed to read chat log {}: {}", src_path.display(), e))?;
                let reader = BufReader::new(file);
                for line in reader.lines() {
                    if matches.len() >= limit {
                        break;
                    }
                    let line = line.map_err(|e| format!("Failed to read chat log: {}", e))?;
                    if line.trim().is_empty() {
                        continue;
                    }
                    let message = match serde_json::from_str::<ChatMessage>(&line) {
                        Ok(m) => m,
                        Err(_) => continue,
                    };
                    if message.timestamp < start_ms || message.timestamp > end_ms {
                        continue;
                    }

                    let username = message.username.to_lowercase();
                    let text = message.message.to_lowercase();
                    if username.contains(&query) || text.contains(&query) {
                        matches.push(message);
                    }
                }
            }

            Ok(json!(matches))
        }
        "disconnect_chat" => {
            let platform: ChatPlatform = get_arg(&payload, "platform")?;
            state.chat_manager.disconnect(platform).await?;
            Ok(Value::Null)
        }
        "retry_chat_connection" => {
            let platform: ChatPlatform = get_arg(&payload, "platform")?;
            let settings = state.settings_manager.load()?;

            if state.ffmpeg_handler.active_count() == 0 {
                return Err("Cannot reconnect chat when no stream is active".to_string());
            }

            match platform {
                ChatPlatform::Twitch => {
                    if settings.twitch_oauth_access_token.is_empty() || settings.chat_twitch_channel.is_empty() {
                        return Err("Twitch chat is not configured".to_string());
                    }
                    connect_twitch_chat(&state.chat_manager, &settings, &state.event_bus).await;
                }
                ChatPlatform::YouTube => {
                    if settings.chat_youtube_channel_id.is_empty()
                        || (settings.youtube_oauth_access_token.is_empty()
                            && settings.chat_youtube_api_key.is_empty())
                    {
                        return Err("YouTube chat is not configured".to_string());
                    }
                    tokio::spawn(connect_youtube_chat_with_retry(
                        state.chat_manager.clone(),
                        state.settings_manager.clone(),
                        state.oauth_service.clone(),
                        state.event_bus.clone(),
                        state.ffmpeg_handler.clone(),
                    ));
                }
                _ => return Err("Retry not supported for this platform".to_string()),
            }

            Ok(Value::Null)
        }
        "disconnect_all_chat" => {
            state.chat_manager.disconnect_all().await?;
            Ok(Value::Null)
        }
        "get_chat_status" => {
            let status = state.chat_manager.get_status().await;
            Ok(json!(status))
        }
        "chat_get_log_status" => {
            let start_ms = state.chat_manager.log_session_start_ms();
            Ok(json!({
                "active": start_ms.is_some(),
                "startedAt": start_ms
            }))
        }
        "get_platform_chat_status" => {
            let platform: ChatPlatform = get_arg(&payload, "platform")?;
            let status = state.chat_manager.get_platform_status(platform).await;
            Ok(json!(status))
        }
        "is_chat_connected" => {
            let connected = state.chat_manager.is_any_connected().await;
            Ok(json!(connected))
        }

        // ============================================================================
        // OAuth Commands
        // ============================================================================
        "oauth_is_configured" => {
            let provider: String = get_arg(&payload, "provider")?;
            // Always configured now with embedded client IDs
            let configured = state.oauth_service.is_configured(&provider).await;
            Ok(json!(configured))
        }
        "oauth_start_flow" => {
            let provider: String = get_arg(&payload, "provider")?;
            let result = state.oauth_service.start_flow(&provider).await?;

            // Start the local callback server to receive the OAuth redirect
            let (callback_server, mut callback_rx) =
                OAuthCallbackServer::start(result.callback_port).await.map_err(|e| {
                    format!("Failed to start OAuth callback server: {e}")
                })?;

            let oauth_service = state.oauth_service.clone();
            let settings_manager = state.settings_manager.clone();
            let event_bus = state.event_bus.clone();
            let provider_name = provider.clone();

            tokio::spawn(async move {
                let timeout = tokio::time::sleep(std::time::Duration::from_secs(180));
                tokio::pin!(timeout);

                let callback = tokio::select! {
                    res = &mut callback_rx => res.ok(),
                    _ = &mut timeout => None,
                };

                match callback {
                    Some(OAuthCallback::Success { code, state }) => {
                        match oauth_service.complete_flow(&provider_name, &code, &state).await {
                            Ok(result) => {
                                let mut settings = match settings_manager.load() {
                                    Ok(settings) => settings,
                                    Err(err) => {
                                        log::error!("Failed to load settings after OAuth: {err}");
                                        callback_server.shutdown();
                                        return;
                                    }
                                };

                                let now = chrono::Utc::now().timestamp();
                                let expires_at = result.tokens.expires_in.map(|e| now + e as i64).unwrap_or(0);

                                match provider_name.as_str() {
                                    "twitch" => {
                                        settings.twitch_oauth_access_token = result.tokens.access_token.clone();
                                        settings.twitch_oauth_refresh_token =
                                            result.tokens.refresh_token.clone().unwrap_or_default();
                                        settings.twitch_oauth_expires_at = expires_at;
                                        settings.twitch_oauth_user_id = result.user_info.user_id.clone();
                                        settings.twitch_oauth_username = result.user_info.username.clone();
                                        settings.twitch_oauth_display_name = result.user_info.display_name.clone();
                                        if settings.chat_twitch_channel.is_empty() {
                                            settings.chat_twitch_channel = result.user_info.username.clone();
                                        }
                                    }
                                    "youtube" => {
                                        settings.youtube_oauth_access_token = result.tokens.access_token.clone();
                                        settings.youtube_oauth_refresh_token =
                                            result.tokens.refresh_token.clone().unwrap_or_default();
                                        settings.youtube_oauth_expires_at = expires_at;
                                        settings.youtube_oauth_channel_id = result.user_info.user_id.clone();
                                        settings.youtube_oauth_channel_name = result.user_info.display_name.clone();
                                        if settings.chat_youtube_channel_id.is_empty() {
                                            settings.chat_youtube_channel_id = result.user_info.user_id.clone();
                                        }
                                    }
                                    _ => {}
                                }

                                if let Err(err) = settings_manager.save(&settings) {
                                    log::error!("Failed to save OAuth settings: {err}");
                                } else {
                                    event_bus.emit("oauth_complete", json!(result.user_info));
                                }
                            }
                            Err(err) => {
                                log::error!("OAuth completion failed for {provider_name}: {err}");
                            }
                        }
                    }
                    Some(OAuthCallback::ImplicitSuccess { access_token, state: _state }) => {
                        // Implicit flow (legacy) -- token received directly, no exchange needed
                        log::info!("Implicit OAuth flow completed for {provider_name}");

                        // Fetch user info using the access token
                        let user_info_result = match provider_name.as_str() {
                            "twitch" => oauth_service.fetch_twitch_user(&access_token).await.map(|u| {
                                spiritstream_server::services::OAuthUserInfo {
                                    provider: "twitch".to_string(),
                                    user_id: u.id,
                                    username: u.login,
                                    display_name: u.display_name,
                                }
                            }),
                            _ => Err("Implicit flow not supported for this provider".to_string()),
                        };

                        match user_info_result {
                            Ok(user_info) => {
                                let mut settings = match settings_manager.load() {
                                    Ok(s) => s,
                                    Err(err) => {
                                        log::error!("Failed to load settings after OAuth: {err}");
                                        callback_server.shutdown();
                                        return;
                                    }
                                };

                                // Implicit flow tokens don't have refresh tokens or expiry
                                settings.twitch_oauth_access_token = access_token;
                                settings.twitch_oauth_refresh_token.clear();
                                settings.twitch_oauth_expires_at = 0;
                                settings.twitch_oauth_user_id = user_info.user_id.clone();
                                settings.twitch_oauth_username = user_info.username.clone();
                                settings.twitch_oauth_display_name = user_info.display_name.clone();
                                if settings.chat_twitch_channel.is_empty() {
                                    settings.chat_twitch_channel = user_info.username.clone();
                                }

                                if let Err(err) = settings_manager.save(&settings) {
                                    log::error!("Failed to save OAuth settings: {err}");
                                } else {
                                    event_bus.emit("oauth_complete", json!(user_info));
                                }
                            }
                            Err(err) => {
                                log::error!("Failed to fetch user info for {provider_name}: {err}");
                            }
                        }
                    }
                    Some(OAuthCallback::Error { error, description }) => {
                        if let Some(description) = description {
                            log::warn!("OAuth callback error for {provider_name}: {error} ({description})");
                        } else {
                            log::warn!("OAuth callback error for {provider_name}: {error}");
                        }
                    }
                    None => {
                        log::warn!("OAuth callback timed out for {provider_name}");
                    }
                }

                callback_server.shutdown();
            });

            // Open the auth URL in the default browser
            if let Err(e) = opener::open(&result.auth_url) {
                log::warn!("Failed to open browser: {}. URL: {}", e, result.auth_url);
            }

            Ok(json!(result))
        }
        "oauth_complete_flow" => {
            // Complete OAuth flow: exchange code, fetch user info, store tokens
            let provider: String = get_arg(&payload, "provider")?;
            let code: String = get_arg(&payload, "code")?;
            let oauth_state: String = get_arg(&payload, "state")?;

            // Complete the flow (exchange code + fetch user info)
            let result = state.oauth_service.complete_flow(&provider, &code, &oauth_state).await?;

            // Store tokens and user info in settings
            let mut settings = state.settings_manager.load()?;
            let now = chrono::Utc::now().timestamp();
            let expires_at = result.tokens.expires_in.map(|e| now + e as i64).unwrap_or(0);

            match provider.as_str() {
                "twitch" => {
                    settings.twitch_oauth_access_token = result.tokens.access_token.clone();
                    settings.twitch_oauth_refresh_token = result.tokens.refresh_token.clone().unwrap_or_default();
                    settings.twitch_oauth_expires_at = expires_at;
                    settings.twitch_oauth_user_id = result.user_info.user_id.clone();
                    settings.twitch_oauth_username = result.user_info.username.clone();
                    settings.twitch_oauth_display_name = result.user_info.display_name.clone();
                    // Also set the channel to the logged-in user by default
                    if settings.chat_twitch_channel.is_empty() {
                        settings.chat_twitch_channel = result.user_info.username.clone();
                    }
                }
                "youtube" => {
                    settings.youtube_oauth_access_token = result.tokens.access_token.clone();
                    settings.youtube_oauth_refresh_token = result.tokens.refresh_token.clone().unwrap_or_default();
                    settings.youtube_oauth_expires_at = expires_at;
                    settings.youtube_oauth_channel_id = result.user_info.user_id.clone();
                    settings.youtube_oauth_channel_name = result.user_info.display_name.clone();
                    // Also set the channel ID for chat
                    if settings.chat_youtube_channel_id.is_empty() {
                        settings.chat_youtube_channel_id = result.user_info.user_id.clone();
                    }
                }
                _ => {}
            }

            state.settings_manager.save(&settings)?;

            // Emit event for frontend
            state.event_bus.sender.send(ServerEvent {
                event: "oauth_complete".to_string(),
                payload: json!(result.user_info),
            }).ok();

            Ok(json!(result.user_info))
        }
        "oauth_get_account" => {
            // Get stored OAuth account info for a provider
            let provider: String = get_arg(&payload, "provider")?;
            let settings = state.settings_manager.load()?;

            let account = match provider.as_str() {
                "twitch" => {
                    if !settings.twitch_oauth_username.is_empty() {
                        json!({
                            "loggedIn": true,
                            "userId": settings.twitch_oauth_user_id,
                            "username": settings.twitch_oauth_username,
                            "displayName": settings.twitch_oauth_display_name
                        })
                    } else {
                        json!({ "loggedIn": false })
                    }
                }
                "youtube" => {
                    if !settings.youtube_oauth_channel_id.is_empty() {
                        json!({
                            "loggedIn": true,
                            "userId": settings.youtube_oauth_channel_id,
                            "username": settings.youtube_oauth_channel_id,
                            "displayName": settings.youtube_oauth_channel_name
                        })
                    } else {
                        json!({ "loggedIn": false })
                    }
                }
                _ => json!({ "loggedIn": false })
            };

            Ok(account)
        }
        "oauth_disconnect" => {
            // Clear OAuth tokens but don't revoke (user might reconnect)
            let provider: String = get_arg(&payload, "provider")?;
            let mut settings = state.settings_manager.load()?;

            match provider.as_str() {
                "twitch" => {
                    settings.twitch_oauth_access_token.clear();
                    settings.twitch_oauth_refresh_token.clear();
                    settings.twitch_oauth_expires_at = 0;
                    settings.twitch_oauth_user_id.clear();
                    settings.twitch_oauth_username.clear();
                    settings.twitch_oauth_display_name.clear();
                }
                "youtube" => {
                    settings.youtube_oauth_access_token.clear();
                    settings.youtube_oauth_refresh_token.clear();
                    settings.youtube_oauth_expires_at = 0;
                    settings.youtube_oauth_channel_id.clear();
                    settings.youtube_oauth_channel_name.clear();
                }
                _ => return Err(format!("Unknown provider: {}", provider))
            }

            state.settings_manager.save(&settings)?;
            Ok(Value::Null)
        }
        "oauth_forget" => {
            // Revoke tokens AND clear from settings
            let provider: String = get_arg(&payload, "provider")?;
            let settings = state.settings_manager.load()?;

            // Try to revoke the token (best effort)
            let token = match provider.as_str() {
                "twitch" => &settings.twitch_oauth_access_token,
                "youtube" => &settings.youtube_oauth_access_token,
                _ => return Err(format!("Unknown provider: {}", provider))
            };

            if !token.is_empty() {
                if let Err(e) = state.oauth_service.revoke_token(&provider, token).await {
                    log::warn!("Failed to revoke {} token: {}", provider, e);
                }
            }

            // Clear from settings (same as disconnect + clear channel config)
            let mut settings = state.settings_manager.load()?;
            match provider.as_str() {
                "twitch" => {
                    settings.twitch_oauth_access_token.clear();
                    settings.twitch_oauth_refresh_token.clear();
                    settings.twitch_oauth_expires_at = 0;
                    settings.twitch_oauth_user_id.clear();
                    settings.twitch_oauth_username.clear();
                    settings.twitch_oauth_display_name.clear();
                    settings.chat_twitch_channel.clear();
                }
                "youtube" => {
                    settings.youtube_oauth_access_token.clear();
                    settings.youtube_oauth_refresh_token.clear();
                    settings.youtube_oauth_expires_at = 0;
                    settings.youtube_oauth_channel_id.clear();
                    settings.youtube_oauth_channel_name.clear();
                    settings.chat_youtube_channel_id.clear();
                }
                _ => {}
            }

            state.settings_manager.save(&settings)?;
            Ok(Value::Null)
        }
        "oauth_refresh_token" => {
            let provider: String = get_arg(&payload, "provider")?;
            let refresh_token: String = get_arg(&payload, "refreshToken")?;
            let tokens = state.oauth_service.refresh_token(&provider, &refresh_token).await?;
            Ok(json!(tokens))
        }
        "oauth_get_config" => {
            // Always configured now with embedded client IDs
            Ok(json!({
                "twitchConfigured": true,
                "youtubeConfigured": true
            }))
        }
        "oauth_set_config" => {
            // Still allow users to override with their own credentials if desired
            let config: OAuthConfig = get_arg(&payload, "config")?;
            state.oauth_service.update_config(config).await;
            Ok(Value::Null)
        }

        _ => Err(format!("Unknown command: {command}")),
    };

    // Only log errors for non-quiet commands, or for unexpected errors
    if let Err(ref e) = result {
        if !QUIET_COMMANDS.contains(&command) || e.contains("Unknown command") {
            log::error!("[invoke_command] Error for {}: {}", command, e);
        }
    }
    result
}

// ============================================================================
// Argument Parsing
// ============================================================================

fn get_arg<T: DeserializeOwned>(payload: &Value, key: &str) -> Result<T, String> {
    let obj = payload
        .as_object()
        .ok_or_else(|| "Invalid payload".to_string())?;
    let value = obj
        .get(key)
        .ok_or_else(|| format!("Missing argument: {key}"))?;
    serde_json::from_value(value.clone()).map_err(|e| format!("Invalid {key}: {e}"))
}

fn get_opt_arg<T: DeserializeOwned>(payload: &Value, key: &str) -> Result<Option<T>, String> {
    let obj = payload
        .as_object()
        .ok_or_else(|| "Invalid payload".to_string())?;
    let value = match obj.get(key) {
        Some(value) => value.clone(),
        None => return Ok(None),
    };

    if value.is_null() {
        return Ok(None);
    }

    serde_json::from_value(value)
        .map(Some)
        .map_err(|e| format!("Invalid {key}: {e}"))
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

fn init_logger(log_dir: &std::path::Path, event_bus: EventBus) -> Result<(), Box<dyn std::error::Error>> {
    let logger = ServerLogger::new(log_dir, event_bus)?;
    log::set_boxed_logger(Box::new(logger))?;
    log::set_max_level(LevelFilter::Info);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file if present (ignore if missing)
    dotenvy::dotenv().ok();

    // Load configuration from environment
    let data_dir = env::var("SPIRITSTREAM_DATA_DIR").unwrap_or_else(|_| "data".to_string());
    let log_dir = env::var("SPIRITSTREAM_LOG_DIR")
        .unwrap_or_else(|_| format!("{data_dir}/logs"));
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
    std::fs::create_dir_all(&app_data_dir)?;
    std::fs::create_dir_all(&log_dir_path)?;

    let profile_manager = Arc::new(ProfileManager::new(app_data_dir.clone()));
    let settings_manager = Arc::new(SettingsManager::new(app_data_dir.clone()));

    // Load settings
    let settings = settings_manager.load().ok();
    let settings_ui_enabled = settings
        .as_ref()
        .map(|settings| settings.backend_ui_enabled)
        .unwrap_or(false);
    let env_ui_enabled = env::var("SPIRITSTREAM_UI_ENABLED")
        .ok()
        .and_then(|value| parse_bool(&value));
    let ui_enabled = env_ui_enabled.unwrap_or(settings_ui_enabled);
    let settings_auth_token = settings.as_ref().and_then(|settings| {
        let trimmed = settings.backend_token.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });
    let auth_token = env_auth_token.or(settings_auth_token);

    // Determine host/port: env vars take precedence, then settings, then defaults
    // If remote access is disabled in settings, force localhost regardless
    let (host, port) = {
        let remote_enabled = settings
            .as_ref()
            .map(|s| s.backend_remote_enabled)
            .unwrap_or(false);
        let settings_host = settings
            .as_ref()
            .map(|s| s.backend_host.clone())
            .unwrap_or_else(|| "127.0.0.1".to_string());
        let settings_port = settings
            .as_ref()
            .map(|s| s.backend_port)
            .unwrap_or(8008);

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

    let ffmpeg_handler = Arc::new(FFmpegHandler::new_with_custom_path(
        app_data_dir.clone(),
        custom_ffmpeg_path,
    ));

    let event_bus = EventBus::new();
    init_logger(&log_dir_path, event_bus.clone())?;

    // Log the themes directory configuration
    let themes_path = PathBuf::from(&themes_dir);
    let themes_exist = themes_path.exists();
    let env_was_set = env::var("SPIRITSTREAM_THEMES_DIR").is_ok();
    log::info!(
        "Themes directory: {themes_dir} (exists={themes_exist}, env_set={env_was_set})"
    );
    if !themes_exist {
        log::warn!("Themes directory does not exist - custom themes may not load");
    }

    let theme_manager = Arc::new(ThemeManager::new(app_data_dir.clone(), PathBuf::from(&themes_dir)));

    // Sync themes and verify sync worked
    log::info!("Starting theme sync from {themes_dir:?} to user data");
    theme_manager.sync_project_themes();

    // Verify sync worked by listing available themes
    let synced_themes = theme_manager.list_themes();
    log::info!(
        "Theme sync complete. Available themes ({}): {:?}",
        synced_themes.len(),
        synced_themes.iter().map(|t| &t.id).collect::<Vec<_>>()
    );

    let theme_event_sink: Arc<dyn EventSink> = Arc::new(event_bus.clone());
    theme_manager.start_watcher(theme_event_sink);

    // Initialize rate limiter
    let rate_limit = env::var("SPIRITSTREAM_RATE_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_RATE_LIMIT_PER_MINUTE);
    let rate_limiter = Arc::new(RateLimiter::direct(Quota::per_minute(
        NonZeroU32::new(rate_limit).unwrap_or(NonZeroU32::new(100).unwrap()),
    )));

    // Get home directory for path validation
    let home_dir = dirs_next::home_dir();

    // Initialize OBS WebSocket handler
    let obs_handler = Arc::new(ObsWebSocketHandler::new(app_data_dir.clone()));

    // Load OBS config from settings if available
    if let Some(ref settings) = settings {
        let obs_config = ObsConfig {
            host: settings.obs_host.clone(),
            port: settings.obs_port,
            password: settings.obs_password.clone(),
            use_auth: settings.obs_use_auth,
            direction: match settings.obs_direction {
                ObsIntegrationDirection::ObsToSpiritstream => {
                    spiritstream_server::services::IntegrationDirection::ObsToSpiritstream
                }
                ObsIntegrationDirection::SpiritstreamToObs => {
                    spiritstream_server::services::IntegrationDirection::SpiritstreamToObs
                }
                ObsIntegrationDirection::Bidirectional => {
                    spiritstream_server::services::IntegrationDirection::Bidirectional
                }
                ObsIntegrationDirection::Disabled => {
                    spiritstream_server::services::IntegrationDirection::Disabled
                }
            },
            auto_connect: settings.obs_auto_connect,
        };
        // Block on setting config since we're in async main
        obs_handler.set_config(obs_config).await;
    }

    // Initialize Discord webhook service
    let discord_service = Arc::new(DiscordWebhookService::new());

    // Initialize Chat manager
    let chat_event_sink: Arc<dyn EventSink> = Arc::new(event_bus.clone());
    let chat_manager = Arc::new(ChatManager::new(chat_event_sink, log_dir_path.clone()));
    if let Some(ref settings) = settings {
        chat_manager.set_crosspost_enabled(settings.chat_crosspost_enabled);
        chat_manager.set_send_enabled(ChatPlatform::Twitch, settings.chat_twitch_send_enabled).await;
        chat_manager.set_send_enabled(ChatPlatform::YouTube, settings.chat_youtube_send_enabled).await;
    }

    // Initialize OAuth service
    let oauth_service = Arc::new(OAuthService::new(OAuthConfig::default()));

    // Start background YouTube token refresh task
    start_youtube_token_refresh_task(
        chat_manager.clone(),
        settings_manager.clone(),
        oauth_service.clone(),
    )
    .await;

    // Start chat reconnect task (stream-tied)
    start_chat_reconnect_task(
        chat_manager.clone(),
        settings_manager.clone(),
        oauth_service.clone(),
        event_bus.clone(),
        ffmpeg_handler.clone(),
    )
    .await;

    let state = AppState {
        profile_manager,
        settings_manager,
        ffmpeg_handler,
        ffmpeg_downloader: Arc::new(AsyncMutex::new(FFmpegDownloader::new())),
        theme_manager,
        obs_handler,
        discord_service,
        chat_manager,
        oauth_service,
        event_bus,
        log_dir: log_dir_path,
        app_data_dir,
        auth_token,
        rate_limiter,
        home_dir,
    };

    // Build CORS layer
    let cors = build_cors_layer();

    // Build CSP header
    let csp_value = HeaderValue::from_static(
        "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; \
         connect-src 'self' ws://localhost:* wss://localhost:* http://localhost:* http://127.0.0.1:*; \
         img-src 'self' data:; font-src 'self'"
    );

    // Build router with security layers
    // Protected routes (require authentication)
    let protected_routes = Router::new()
        .route("/api/invoke/:command", post(invoke))
        .route("/ws", get(ws_handler))
        // File browser endpoints for HTTP mode dialogs
        .route("/api/files/browse", get(files_browse))
        .route("/api/files/home", get(files_home))
        .route("/api/files/open", post(files_open))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    // Public routes (no auth required)
    let public_routes = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/auth/login", post(auth_login))
        .route("/auth/logout", post(auth_logout))
        .route("/auth/check", get(auth_check));

    // Combine all routes
    let mut app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state.clone(), rate_limit_middleware))
        .layer(CookieManagerLayer::new())
        .layer(cors)
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CONTENT_SECURITY_POLICY,
            csp_value,
        ));

    // Optionally serve static UI files
    let ui_path = PathBuf::from(ui_dir);
    if ui_enabled && ui_path.exists() {
        app = app.fallback_service(
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
    axum::serve(listener, app).await?;

    Ok(())
}

