use axum::{
    body::Body,
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
use futures_util::StreamExt;
use tokio_stream::wrappers::BroadcastStream;
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use log::{Level, LevelFilter, Log, Metadata, Record};
use chrono::Local;
use std::{
    collections::HashMap,
    env,
    fs::OpenOptions,
    io::Write,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    num::NonZeroU32,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use subtle::ConstantTimeEq;
use tokio::sync::{broadcast, Mutex as AsyncMutex};
use tokio::signal;
use tower_cookies::{Cookie, CookieManagerLayer, Cookies};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
};

use spiritstream_server::commands::{get_encoders, test_ffmpeg, test_rtmp_target, validate_ffmpeg_path};
use spiritstream_server::models::{
    OutputGroup, Profile, RtmpInput, Settings, Source, Scene, SourceLayer, Transform, AudioTrack,
    AudioDeviceSource, AudioInputDevice,
};
use spiritstream_server::services::{
    prune_logs, read_recent_logs, validate_extension, validate_path_within_any,
    Encryption, EventSink, FFmpegDownloader, FFmpegHandler, ProfileManager, SettingsManager,
    ThemeManager, DeviceDiscovery, PreviewHandler, PreviewParams,
    // Native capture services
    ScreenCaptureService, ScreenCaptureConfig, AudioCaptureService, AudioCaptureConfig,
    CameraCaptureService, CameraCaptureConfig,
    NativePreviewService,
    RecordingService, RecordingConfig, RecordingFormat,
    ReplayBufferService, ReplayBufferConfig,
    CaptureIndicatorService, CaptureType,
    PermissionsService,
    // WebRTC services
    Go2RtcManager, unavailable_webrtc_info,
    // H264 capture service for native screen capture to WebRTC
    H264CaptureService,
    // Audio level monitoring
    AudioLevelService,
    // Audio level extraction from FFmpeg-based sources
    AudioLevelExtractor,
};
// ScreenCaptureKit audio capture service (macOS only)
#[cfg(target_os = "macos")]
use spiritstream_server::services::SckAudioCaptureService;

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
    preview_handler: Arc<PreviewHandler>,
    event_bus: EventBus,
    log_dir: PathBuf,
    app_data_dir: PathBuf,
    auth_token: Option<String>,
    rate_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
    // Allowed export directories for path validation
    home_dir: Option<PathBuf>,
    // Native capture services
    screen_capture: Arc<ScreenCaptureService>,
    audio_capture: Arc<AudioCaptureService>,
    camera_capture: Arc<CameraCaptureService>,
    native_preview: Arc<NativePreviewService>,
    recording_service: Arc<RecordingService>,
    replay_buffer: Arc<ReplayBufferService>,
    capture_indicator: Arc<CaptureIndicatorService>,
    // WebRTC preview service
    go2rtc_manager: Arc<Go2RtcManager>,
    // H264 capture service for native screen capture → WebRTC
    h264_capture: Arc<H264CaptureService>,
    // Audio level monitoring service
    audio_level_service: Arc<AudioLevelService>,
    // Audio level extractor for FFmpeg-based sources (MediaFile, RTMP, ScreenCapture, etc.)
    audio_level_extractor: Arc<AudioLevelExtractor>,
    // ScreenCaptureKit audio capture for macOS (screen, window, game capture audio)
    #[cfg(target_os = "macos")]
    sck_audio_capture: Arc<SckAudioCaptureService>,
    // Server port for constructing HTTP URLs
    server_port: u16,
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
            let _ = writeln!(file, "{line}");
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

    fn flush(&self) {}
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

/// Sanitize error messages to prevent information disclosure
fn sanitize_error(error: &str) -> String {
    let lower = error.to_lowercase();

    if lower.contains("failed to read") || lower.contains("no such file") || lower.contains("not found") {
        return "Resource not found".to_string();
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

    // Return generic message for unknown errors in production
    // In debug mode, we could log the actual error server-side
    log::debug!("Sanitized error: {error}");
    "Operation failed".to_string()
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
    let mut checks: Vec<(&str, bool)> = Vec::new();

    // Check 1: ProfileManager can access profiles directory
    let profiles_ok = state.profile_manager.get_all_names().await.is_ok();
    checks.push(("profiles", profiles_ok));

    // Check 2: SettingsManager can load settings
    let settings_ok = state.settings_manager.load().is_ok();
    checks.push(("settings", settings_ok));

    // Check 3: ThemeManager initialized (theme list is always available after init)
    let themes_ok = true;
    checks.push(("themes", themes_ok));

    let all_ok = checks.iter().all(|(_, ok)| *ok);
    let failed: Vec<&str> = checks
        .iter()
        .filter(|(_, ok)| !ok)
        .map(|(name, _)| *name)
        .collect();

    if all_ok {
        Json(json!({ "ready": true })).into_response()
    } else {
        log::warn!("Readiness check failed: {failed:?}");
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "ready": false, "failed": failed })),
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

/// GET /api/files/browse - List directory contents
/// Query params: path (optional, defaults to home directory)
async fn files_browse(
    State(state): State<AppState>,
    Query(params): Query<FileBrowseQuery>,
) -> impl IntoResponse {
    // Determine the directory to browse
    let browse_path = match params.path {
        Some(p) if !p.is_empty() => {
            // Expand ~ to home directory
            if p.starts_with("~/") {
                match &state.home_dir {
                    Some(home) => home.join(&p[2..]),
                    None => PathBuf::from(&p),
                }
            } else if p == "~" {
                match &state.home_dir {
                    Some(home) => home.clone(),
                    None => PathBuf::from(&p),
                }
            } else {
                PathBuf::from(&p)
            }
        }
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
    let system_bin_paths: Vec<PathBuf> = if cfg!(target_os = "windows") {
        vec![
            PathBuf::from("C:\\Program Files"),
            PathBuf::from("C:\\Program Files (x86)"),
        ]
    } else {
        vec![
            PathBuf::from("/opt"),        // Homebrew on Apple Silicon
            PathBuf::from("/usr/local"),  // Homebrew on Intel, common installs
            PathBuf::from("/usr/bin"),    // System binaries
        ]
    };

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

/// GET /api/system/default-paths - Get platform-specific default directories
/// Creates the directories if they don't exist
async fn system_default_paths(State(state): State<AppState>) -> impl IntoResponse {
    let home = match &state.home_dir {
        Some(h) => h.clone(),
        None => {
            return Json(json!({
                "ok": false,
                "error": "Cannot determine home directory"
            })).into_response();
        }
    };

    // Determine platform and appropriate video directory
    let (platform, videos_dir) = if cfg!(target_os = "macos") {
        ("macos", home.join("Movies"))
    } else if cfg!(target_os = "windows") {
        ("windows", home.join("Videos"))
    } else {
        ("linux", home.join("Videos"))
    };

    // Create subdirectories for recordings and replays
    let recordings_dir = videos_dir.join("SpiritStream");
    let replays_dir = videos_dir.join("SpiritStream").join("Replays");

    // Ensure directories exist (create if needed)
    if let Err(e) = std::fs::create_dir_all(&recordings_dir) {
        log::warn!("Failed to create recordings directory {:?}: {}", recordings_dir, e);
    }
    if let Err(e) = std::fs::create_dir_all(&replays_dir) {
        log::warn!("Failed to create replays directory {:?}: {}", replays_dir, e);
    }

    Json(json!({
        "ok": true,
        "data": {
            "platform": platform,
            "home": home.to_string_lossy().to_string(),
            "videos": videos_dir.to_string_lossy().to_string(),
            "recordings": recordings_dir.to_string_lossy().to_string(),
            "replays": replays_dir.to_string_lossy().to_string()
        }
    })).into_response()
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
    // Expand ~ to home directory
    let path = if payload.path.starts_with("~/") {
        match &state.home_dir {
            Some(home) => home.join(&payload.path[2..]),
            None => PathBuf::from(&payload.path),
        }
    } else if payload.path == "~" {
        match &state.home_dir {
            Some(home) => home.clone(),
            None => PathBuf::from(&payload.path),
        }
    } else {
        PathBuf::from(&payload.path)
    };

    // Security: Validate path is within allowed directories
    // Include common binary directories for consistency with file browser
    let system_bin_paths: Vec<PathBuf> = if cfg!(target_os = "windows") {
        vec![
            PathBuf::from("C:\\Program Files"),
            PathBuf::from("C:\\Program Files (x86)"),
        ]
    } else {
        vec![
            PathBuf::from("/opt"),
            PathBuf::from("/usr/local"),
            PathBuf::from("/usr/bin"),
        ]
    };

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

// ============================================================================
// Static File Serving Endpoint
// ============================================================================

#[derive(Debug, Deserialize)]
struct StaticFileQuery {
    path: String,
}

/// GET /api/static - Serve a static file (images, HTML) from the file system
/// Query params: path (required, the absolute file path to serve)
async fn static_file_handler(
    State(state): State<AppState>,
    Query(params): Query<StaticFileQuery>,
) -> impl IntoResponse {
    let file_path = PathBuf::from(&params.path);

    // Security: Validate file exists
    if !file_path.exists() {
        return (
            StatusCode::NOT_FOUND,
            [("Content-Type", "text/plain")],
            "File not found".to_string(),
        )
            .into_response();
    }

    // Security: Must be a file, not a directory
    if !file_path.is_file() {
        return (
            StatusCode::BAD_REQUEST,
            [("Content-Type", "text/plain")],
            "Path is not a file".to_string(),
        )
            .into_response();
    }

    // Security: Validate path is within allowed directories (home or app data)
    let mut allowed_dirs: Vec<&std::path::Path> = vec![state.app_data_dir.as_path()];
    if let Some(ref home) = state.home_dir {
        allowed_dirs.push(home.as_path());
    }
    // Also allow common media directories
    #[cfg(target_os = "macos")]
    {
        if let Some(ref home) = state.home_dir {
            let movies = home.join("Movies");
            let pictures = home.join("Pictures");
            let downloads = home.join("Downloads");
            let desktop = home.join("Desktop");
            let documents = home.join("Documents");
            // Check if file is under any of these
            let is_allowed = [&movies, &pictures, &downloads, &desktop, &documents]
                .iter()
                .any(|dir| file_path.starts_with(dir));
            if !is_allowed && !allowed_dirs.iter().any(|d| file_path.starts_with(d)) {
                return (
                    StatusCode::FORBIDDEN,
                    [("Content-Type", "text/plain")],
                    "Access denied".to_string(),
                )
                    .into_response();
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if !allowed_dirs.iter().any(|d| file_path.starts_with(d)) {
            return (
                StatusCode::FORBIDDEN,
                [("Content-Type", "text/plain")],
                "Access denied".to_string(),
            )
                .into_response();
        }
    }

    // Read the file
    let content = match std::fs::read(&file_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!("Failed to read static file {:?}: {}", file_path, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [("Content-Type", "text/plain")],
                "Failed to read file".to_string(),
            )
                .into_response();
        }
    };

    // Determine content type based on file extension
    let content_type = match file_path.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("bmp") => "image/bmp",
        Some("html") | Some("htm") => "text/html",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        _ => "application/octet-stream",
    };

    // Build response with appropriate headers
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", content_type)
        .header("Cache-Control", "public, max-age=3600");

    // For HTML files, add a permissive CSP to allow external resources (fonts, stylesheets, etc.)
    // This is necessary because user-provided HTML content may include external dependencies
    if content_type == "text/html" {
        response = response.header(
            "Content-Security-Policy",
            "default-src * 'unsafe-inline' 'unsafe-eval' data: blob:; \
             script-src * 'unsafe-inline' 'unsafe-eval'; \
             style-src * 'unsafe-inline' https:; \
             font-src * data: https:; \
             img-src * data: blob: https: http:; \
             connect-src * https: http: ws: wss:"
        );
    }

    response.body(Body::from(content)).unwrap().into_response()
}

// ============================================================================
// Preview Endpoints
// ============================================================================

#[derive(Debug, Deserialize)]
struct PreviewQuery {
    width: Option<u32>,
    height: Option<u32>,
    fps: Option<u32>,
    quality: Option<u32>,
}

impl From<PreviewQuery> for PreviewParams {
    fn from(query: PreviewQuery) -> Self {
        let defaults = PreviewParams::default();
        PreviewParams {
            width: query.width.unwrap_or(defaults.width).min(1280), // Cap at 720p width
            height: query.height.unwrap_or(defaults.height).min(720),
            fps: query.fps.unwrap_or(defaults.fps).min(30).max(5),
            quality: query.quality.unwrap_or(defaults.quality).clamp(1, 15),
        }
    }
}

/// GET /api/preview/source/:source_id - Stream MJPEG preview for a source
async fn source_preview_handler(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
    Query(query): Query<PreviewQuery>,
    cookies: Cookies,
    headers: HeaderMap,
) -> impl IntoResponse {
    log::info!("Preview request for source: {}", source_id);

    // Authentication check (note: also accept token in query param for img tags)
    if let Some(expected) = state.auth_token.as_deref() {
        let authenticated = cookies.get(AUTH_COOKIE_NAME).is_some()
            || bearer_token(&headers).is_some_and(|t| verify_token(expected, t));

        if !authenticated {
            log::warn!("Preview request unauthorized for source: {}", source_id);
            return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
        }
    }

    let params: PreviewParams = query.into();
    log::debug!("Preview params: {}x{} @ {} fps, quality {}", params.width, params.height, params.fps, params.quality);

    // Find source from active profile
    let source = {
        // Get last profile from settings
        let settings = match state.settings_manager.load() {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to load settings for preview: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load settings").into_response();
            }
        };

        let profile_name = match settings.last_profile.as_ref() {
            Some(name) => name.clone(),
            None => {
                log::warn!("No active profile for preview request");
                return (StatusCode::BAD_REQUEST, "No active profile").into_response();
            }
        };

        log::debug!("Loading profile '{}' for preview", profile_name);

        // Load profile (without password for preview - encrypted profiles not supported in preview yet)
        match state.profile_manager.load(&profile_name, None).await {
            Ok(profile) => {
                log::debug!("Profile has {} sources", profile.sources.len());
                profile.sources.into_iter().find(|s| s.id() == source_id)
            }
            Err(e) => {
                log::error!("Failed to load profile for preview: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load profile").into_response();
            }
        }
    };

    let source = match source {
        Some(s) => {
            log::info!("Found source for preview: {} (type: {:?})", s.name(), s.id());
            s
        }
        None => {
            log::warn!("Source {} not found in profile", source_id);
            return (StatusCode::NOT_FOUND, "Source not found").into_response();
        }
    };

    // Start preview stream
    let rx = match state.preview_handler.start_source_preview(&source, params) {
        Ok(rx) => rx,
        Err(e) => {
            log::error!("Failed to start preview: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to start preview: {}", e)).into_response();
        }
    };

    // Convert broadcast receiver to stream
    let stream = BroadcastStream::new(rx);

    // Build MJPEG streaming response using standard multipart format:
    // "--frame\r\nContent-Type: image/jpeg\r\nContent-Length: <len>\r\n\r\n" + jpeg_data + "\r\n"
    let body_stream = stream.filter_map(move |result| {
        async move {
            match result {
                Ok(frame_data) => {
                    // Build multipart MJPEG frame
                    let header = format!(
                        "--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                        frame_data.len()
                    );
                    let mut response = Vec::with_capacity(frame_data.len() + header.len() + 2);
                    response.extend_from_slice(header.as_bytes());
                    response.extend_from_slice(&frame_data);
                    response.extend_from_slice(b"\r\n");
                    Some(Ok::<_, std::io::Error>(axum::body::Bytes::from(response)))
                }
                Err(_) => None, // Skip lagged frames
            }
        }
    });

    let body = Body::from_stream(body_stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "multipart/x-mixed-replace; boundary=frame",
        )
        .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
        .header(header::PRAGMA, "no-cache")
        .header(header::EXPIRES, "0")
        .body(body)
        .unwrap()
        .into_response()
}

/// POST /api/preview/source/:source_id/stop - Stop a source preview
async fn stop_source_preview_handler(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
    cookies: Cookies,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Authentication check
    if let Some(expected) = state.auth_token.as_deref() {
        let authenticated = cookies.get(AUTH_COOKIE_NAME).is_some()
            || bearer_token(&headers).is_some_and(|t| verify_token(expected, t));

        if !authenticated {
            return (StatusCode::UNAUTHORIZED, Json(json!({ "ok": false, "error": "Unauthorized" })));
        }
    }

    state.preview_handler.stop_source_preview(&source_id);
    (StatusCode::OK, Json(json!({ "ok": true })))
}

/// POST /api/preview/stop-all - Stop all previews
async fn stop_all_previews_handler(
    State(state): State<AppState>,
    cookies: Cookies,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Authentication check
    if let Some(expected) = state.auth_token.as_deref() {
        let authenticated = cookies.get(AUTH_COOKIE_NAME).is_some()
            || bearer_token(&headers).is_some_and(|t| verify_token(expected, t));

        if !authenticated {
            return (StatusCode::UNAUTHORIZED, Json(json!({ "ok": false, "error": "Unauthorized" })));
        }
    }

    state.preview_handler.stop_all_previews();
    (StatusCode::OK, Json(json!({ "ok": true })))
}

/// GET /api/preview/source/:source_id/snapshot - Get a single JPEG snapshot
/// More reliable than MJPEG streaming for WebKit-based browsers (Safari, Tauri)
async fn source_snapshot_handler(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
    Query(query): Query<PreviewQuery>,
    cookies: Cookies,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Authentication check
    if let Some(expected) = state.auth_token.as_deref() {
        let authenticated = cookies.get(AUTH_COOKIE_NAME).is_some()
            || bearer_token(&headers).is_some_and(|t| verify_token(expected, t));

        if !authenticated {
            return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
        }
    }

    let params: PreviewParams = query.into();

    // Find source from active profile
    let source = {
        let settings = match state.settings_manager.load() {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to load settings for snapshot: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load settings").into_response();
            }
        };

        let profile_name = match settings.last_profile.as_ref() {
            Some(name) => name.clone(),
            None => {
                return (StatusCode::BAD_REQUEST, "No active profile").into_response();
            }
        };

        match state.profile_manager.load(&profile_name, None).await {
            Ok(profile) => profile.sources.into_iter().find(|s| s.id() == source_id),
            Err(e) => {
                log::error!("Failed to load profile for snapshot: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load profile").into_response();
            }
        }
    };

    let source = match source {
        Some(s) => s,
        None => {
            return (StatusCode::NOT_FOUND, "Source not found").into_response();
        }
    };

    // Capture snapshot (async with timeout to prevent blocking)
    match state.preview_handler.capture_snapshot(&source, &params).await {
        Ok(jpeg_data) => {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "image/jpeg")
                .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
                .body(Body::from(jpeg_data))
                .unwrap()
                .into_response()
        }
        Err(e) => {
            log::warn!("Snapshot capture failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Snapshot failed: {}", e)).into_response()
        }
    }
}

/// GET /api/preview/scene/:profile/:scene_id - Stream MJPEG preview for a composed scene
async fn scene_preview_handler(
    State(state): State<AppState>,
    Path((profile_name, scene_id)): Path<(String, String)>,
    Query(query): Query<PreviewQuery>,
    cookies: Cookies,
    headers: HeaderMap,
) -> impl IntoResponse {
    log::info!("Scene preview request for profile:{} scene:{}", profile_name, scene_id);

    // Authentication check
    if let Some(expected) = state.auth_token.as_deref() {
        let authenticated = cookies.get(AUTH_COOKIE_NAME).is_some()
            || bearer_token(&headers).is_some_and(|t| verify_token(expected, t));

        if !authenticated {
            log::warn!("Scene preview request unauthorized");
            return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
        }
    }

    let params: PreviewParams = query.into();
    log::debug!("Scene preview params: {}x{} @ {} fps, quality {}", params.width, params.height, params.fps, params.quality);

    // Load profile and find scene
    let (scene, sources) = {
        match state.profile_manager.load(&profile_name, None).await {
            Ok(profile) => {
                let scene = profile.scenes.into_iter().find(|s| s.id == scene_id);
                let sources = profile.sources;
                match scene {
                    Some(s) => (s, sources),
                    None => {
                        log::warn!("Scene {} not found in profile {}", scene_id, profile_name);
                        return (StatusCode::NOT_FOUND, "Scene not found").into_response();
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to load profile for scene preview: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load profile").into_response();
            }
        }
    };

    // Start scene preview
    let rx = match state.preview_handler.start_scene_preview(&scene, &sources, params) {
        Ok(rx) => rx,
        Err(e) => {
            log::error!("Failed to start scene preview: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to start preview: {}", e)).into_response();
        }
    };

    // Convert broadcast receiver to stream
    let stream = BroadcastStream::new(rx);

    // Build MJPEG streaming response
    let body_stream = stream.filter_map(move |result| {
        async move {
            match result {
                Ok(frame_data) => {
                    let header = format!(
                        "--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                        frame_data.len()
                    );
                    let mut response = Vec::with_capacity(frame_data.len() + header.len() + 2);
                    response.extend_from_slice(header.as_bytes());
                    response.extend_from_slice(&frame_data);
                    response.extend_from_slice(b"\r\n");
                    Some(Ok::<_, std::io::Error>(axum::body::Bytes::from(response)))
                }
                Err(_) => None,
            }
        }
    });

    let body = Body::from_stream(body_stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "multipart/x-mixed-replace; boundary=frame",
        )
        .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
        .header(header::PRAGMA, "no-cache")
        .header(header::EXPIRES, "0")
        .body(body)
        .unwrap()
        .into_response()
}

/// GET /api/preview/scene/:profile/:scene_id/snapshot - Get a single JPEG snapshot of composed scene
async fn scene_snapshot_handler(
    State(state): State<AppState>,
    Path((profile_name, scene_id)): Path<(String, String)>,
    Query(query): Query<PreviewQuery>,
    cookies: Cookies,
    headers: HeaderMap,
) -> impl IntoResponse {
    log::debug!("Scene snapshot request for profile:{} scene:{}", profile_name, scene_id);

    // Authentication check
    if let Some(expected) = state.auth_token.as_deref() {
        let authenticated = cookies.get(AUTH_COOKIE_NAME).is_some()
            || bearer_token(&headers).is_some_and(|t| verify_token(expected, t));

        if !authenticated {
            return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
        }
    }

    let params: PreviewParams = query.into();

    // Load profile and find scene
    let (scene, sources) = {
        match state.profile_manager.load(&profile_name, None).await {
            Ok(profile) => {
                let scene = profile.scenes.into_iter().find(|s| s.id == scene_id);
                let sources = profile.sources;
                match scene {
                    Some(s) => (s, sources),
                    None => {
                        return (StatusCode::NOT_FOUND, "Scene not found").into_response();
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to load profile for scene snapshot: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load profile").into_response();
            }
        }
    };

    // Capture scene snapshot
    match state.preview_handler.capture_scene_snapshot(&scene, &sources, &params).await {
        Ok(jpeg_data) => {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "image/jpeg")
                .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
                .body(Body::from(jpeg_data))
                .unwrap()
                .into_response()
        }
        Err(e) => {
            log::warn!("Scene snapshot capture failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Snapshot failed: {}", e)).into_response()
        }
    }
}

/// POST /api/preview/scene/stop - Stop the scene preview
async fn stop_scene_preview_handler(
    State(state): State<AppState>,
    cookies: Cookies,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Authentication check
    if let Some(expected) = state.auth_token.as_deref() {
        let authenticated = cookies.get(AUTH_COOKIE_NAME).is_some()
            || bearer_token(&headers).is_some_and(|t| verify_token(expected, t));

        if !authenticated {
            return (StatusCode::UNAUTHORIZED, Json(json!({ "ok": false, "error": "Unauthorized" })));
        }
    }

    state.preview_handler.stop_scene_preview();
    (StatusCode::OK, Json(json!({ "ok": true })))
}

// ============================================================================
// WebRTC Preview Endpoints (go2rtc integration)
// ============================================================================

/// GET /api/webrtc/available - Check if go2rtc WebRTC server is available
async fn webrtc_available_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let available = state.go2rtc_manager.is_available();
    Json(json!({ "ok": true, "data": available }))
}

/// GET /api/webrtc/info/:source_id - Get WebRTC streaming info for a source
async fn webrtc_info_handler(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> impl IntoResponse {
    if !state.go2rtc_manager.is_available() {
        return Json(json!({ "ok": true, "data": unavailable_webrtc_info() }));
    }

    let info = state.go2rtc_manager.client().get_webrtc_info(&source_id);
    Json(json!({ "ok": true, "data": info }))
}

/// POST /api/webrtc/start/:source_id - Register a source with go2rtc for WebRTC streaming
async fn webrtc_start_handler(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
    cookies: Cookies,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Authentication check
    if let Some(expected) = state.auth_token.as_deref() {
        let authenticated = cookies.get(AUTH_COOKIE_NAME).is_some()
            || bearer_token(&headers).is_some_and(|t| verify_token(expected, t));

        if !authenticated {
            return Json(json!({ "ok": false, "error": "Unauthorized" }));
        }
    }

    if !state.go2rtc_manager.is_available() {
        return Json(json!({ "ok": false, "error": "go2rtc is not available" }));
    }

    // Find source from active profile
    let source = {
        let settings = match state.settings_manager.load() {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to load settings for WebRTC: {}", e);
                return Json(json!({ "ok": false, "error": "Failed to load settings" }));
            }
        };

        let profile_name = match settings.last_profile.as_ref() {
            Some(name) => name.clone(),
            None => {
                return Json(json!({ "ok": false, "error": "No active profile" }));
            }
        };

        match state.profile_manager.load(&profile_name, None).await {
            Ok(profile) => profile.sources.into_iter().find(|s| s.id() == source_id),
            Err(e) => {
                log::error!("Failed to load profile for WebRTC: {}", e);
                return Json(json!({ "ok": false, "error": "Failed to load profile" }));
            }
        }
    };

    let source = match source {
        Some(s) => s,
        None => {
            return Json(json!({ "ok": false, "error": "Source not found" }));
        }
    };

    // Build go2rtc source URL based on source type
    // go2rtc uses native source formats like ffmpeg:device for cameras
    let go2rtc_source = match &source {
        Source::Camera(cam) => {
            // Parse device_id - it may be numeric index or device name
            let device_index = cam.device_id.parse::<u32>().unwrap_or(0);
            // Use go2rtc's native ffmpeg:device source
            format!("ffmpeg:device?video={}&video_size=1280x720&framerate=30#video=h264", device_index)
        }
        Source::ScreenCapture(screen) => {
            // Check screen recording permission first (macOS)
            #[cfg(target_os = "macos")]
            {
                // Check if we have screen recording permission
                let has_permission = ScreenCaptureService::has_permission_async().await;
                if !has_permission {
                    // Try to request permission (will show system prompt)
                    let granted = ScreenCaptureService::request_permission_async().await;
                    if !granted {
                        return Json(json!({
                            "ok": false,
                            "error": "Screen Recording permission required. Please grant permission in System Settings > Privacy & Security > Screen Recording, then try again."
                        }));
                    }
                }
            }

            // For screen capture, we use native scap capture + H264 encoding via our server.
            // This approach:
            // 1. Uses scap (which has screen recording permission via our server process)
            // 2. Encodes to H264 MPEG-TS using FFmpeg with bt709 color space
            // 3. Serves the stream via HTTP endpoint
            // 4. go2rtc consumes with #video=copy to PASSTHROUGH (no re-encoding!)
            //
            // The #video=copy flag is critical - it tells go2rtc to not re-encode,
            // preserving our bt709 color space metadata and reducing latency.

            // Start H264 capture if not already running
            // IMPORTANT: start_capture is a blocking function that calls scap::get_all_targets()
            // which can block for 3-10 seconds on macOS. We must run it via spawn_blocking.
            let h264_capture = state.h264_capture.clone();
            let screen_clone = screen.clone();

            let capture_result = tokio::task::spawn_blocking(move || {
                // Wrap in catch_unwind to handle any panics from scap
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    h264_capture.start_capture_http(&screen_clone, None)
                }))
            }).await;

            match capture_result {
                Ok(Ok(Ok(_receiver))) => {
                    // Success - H264 capture started
                    log::debug!("H264 capture started for {} (HTTP mode)", source_id);
                }
                Ok(Ok(Err(e))) => {
                    // start_capture returned an error
                    if !e.contains("Already capturing") {
                        log::error!("Failed to start H264 capture: {}", e);
                        return Json(json!({ "ok": false, "error": format!("Failed to start screen capture: {}", e) }));
                    }
                    log::debug!("H264 capture already running for {}", source_id);
                }
                Ok(Err(_panic)) => {
                    // start_capture panicked
                    log::error!("H264 capture panicked for source {}", source_id);
                    return Json(json!({ "ok": false, "error": "Screen capture failed unexpectedly (panic)" }));
                }
                Err(join_err) => {
                    // spawn_blocking task failed to join
                    log::error!("H264 capture task failed: {}", join_err);
                    return Json(json!({ "ok": false, "error": "Screen capture task failed" }));
                }
            }

            // Build HTTP URL pointing to our MPEG-TS stream endpoint
            // CRITICAL: Add #video=copy to tell go2rtc to passthrough without re-encoding!
            // This preserves bt709 color space and reduces latency.
            let http_url = format!(
                "http://127.0.0.1:{}/api/capture/{}/stream#video=copy",
                state.server_port,
                source_id
            );

            log::info!("Screen capture using HTTP source with passthrough: {}", http_url);

            http_url
        }
        Source::CaptureCard(card) => {
            // Capture cards as video devices
            let device_index = card.device_id.parse::<u32>().unwrap_or(0);
            format!("ffmpeg:device?video={}&video_size=1920x1080&framerate=30#video=h264", device_index)
        }
        Source::AudioDevice(_) => {
            return Json(json!({ "ok": false, "error": "Audio-only sources not supported for WebRTC video" }));
        }
        Source::MediaFile(media) => {
            // Use ffmpeg source for media files
            format!("ffmpeg:{}#video=h264", media.file_path)
        }
        Source::Rtmp(rtmp) => {
            // RTMP sources can be used directly
            format!("rtmp://{}:{}/{}", rtmp.bind_address, rtmp.port, rtmp.application)
        }
        // Client-rendered sources (Color, Text, Browser, MediaPlaylist, NestedScene) are not supported for WebRTC
        // These are rendered purely in the browser and don't need go2rtc
        Source::Color(_) | Source::Text(_) | Source::Browser(_) | Source::MediaPlaylist(_) | Source::NestedScene(_) => {
            return Json(json!({ "ok": false, "error": "Client-rendered sources don't require WebRTC registration" }));
        }
        Source::WindowCapture(win) => {
            // Window capture - similar to screen capture
            #[cfg(target_os = "macos")]
            {
                format!("ffmpeg:device?video={}&framerate={}#video=h264", win.window_id, win.fps)
            }
            #[cfg(not(target_os = "macos"))]
            {
                return Json(json!({ "ok": false, "error": "Window capture not yet implemented for this platform via WebRTC" }));
            }
        }
        Source::GameCapture(_) => {
            // Game capture requires platform-specific implementation
            return Json(json!({ "ok": false, "error": "Game capture not yet implemented for WebRTC streaming" }));
        }
        Source::Ndi(_ndi) => {
            // NDI requires NDI SDK
            return Json(json!({ "ok": false, "error": "NDI source requires NDI SDK to be installed" }));
        }
    };

    log::debug!("Registering go2rtc source '{}': {}", source_id, go2rtc_source);

    match state.go2rtc_manager.register_source(&source_id, &go2rtc_source).await {
        Ok(_) => {
            let info = state.go2rtc_manager.client().get_webrtc_info(&source_id);
            Json(json!({ "ok": true, "data": info }))
        }
        Err(e) => {
            log::error!("Failed to register source with go2rtc: {}", e);
            Json(json!({ "ok": false, "error": e }))
        }
    }
}

/// POST /api/webrtc/stop/:source_id - Unregister a source from go2rtc
async fn webrtc_stop_handler(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
    cookies: Cookies,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Authentication check
    if let Some(expected) = state.auth_token.as_deref() {
        let authenticated = cookies.get(AUTH_COOKIE_NAME).is_some()
            || bearer_token(&headers).is_some_and(|t| verify_token(expected, t));

        if !authenticated {
            return Json(json!({ "ok": false, "error": "Unauthorized" }));
        }
    }

    if let Err(e) = state.go2rtc_manager.unregister_source(&source_id).await {
        log::warn!("Failed to unregister source from go2rtc: {}", e);
    }

    // Also stop H264 capture if it was running for this source
    if state.h264_capture.is_capturing(&source_id) {
        if let Err(e) = state.h264_capture.stop_capture(&source_id) {
            log::warn!("Failed to stop H264 capture for {}: {}", source_id, e);
        }
    }

    Json(json!({ "ok": true }))
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

/// WebSocket handler for preview JPEG frame streaming
/// GET /ws/preview/{source_id}
async fn ws_preview_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
    Query(query): Query<AuthQuery>,
    cookies: Cookies,
) -> impl IntoResponse {
    // Check authentication
    let authenticated = state.auth_token.is_none()
        || cookies.get(AUTH_COOKIE_NAME).is_some()
        || query.token.as_deref().is_some_and(|token| {
            state.auth_token.as_deref().is_some_and(|expected| verify_token(expected, token))
        });

    if !authenticated {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }

    // Check if preview exists
    if !state.native_preview.has_preview(&source_id) {
        return (StatusCode::NOT_FOUND, format!("Preview not found: {}", source_id)).into_response();
    }

    // Subscribe to the preview
    let receiver = match state.native_preview.subscribe_preview(&source_id) {
        Some(rx) => rx,
        None => return (StatusCode::NOT_FOUND, "Preview no longer available").into_response(),
    };

    ws.on_upgrade(move |socket| handle_preview_socket(socket, receiver))
}

async fn handle_preview_socket(mut socket: WebSocket, mut receiver: broadcast::Receiver<bytes::Bytes>) {
    while let Ok(frame) = receiver.recv().await {
        // Send JPEG frame as binary
        if socket.send(Message::Binary(frame.to_vec())).await.is_err() {
            break;
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

async fn invoke_command(
    state: &AppState,
    command: &str,
    payload: Value,
) -> Result<Value, String> {
    match command {
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

            // Update last_profile in settings so preview handler knows which profile is active
            if let Ok(mut settings) = state.settings_manager.load() {
                settings.last_profile = Some(name.clone());
                let _ = state.settings_manager.save(&settings);
            }

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
            let event_sink: Arc<dyn EventSink> = Arc::new(state.event_bus.clone());
            let pid = state.ffmpeg_handler.start(&group, &incoming_url, event_sink)?;
            Ok(json!(pid))
        }
        "start_all_streams" => {
            let groups: Vec<OutputGroup> = get_arg(&payload, "groups")?;
            let incoming_url: String = get_arg(&payload, "incomingUrl")?;
            let event_sink: Arc<dyn EventSink> = Arc::new(state.event_bus.clone());
            let pids = state.ffmpeg_handler.start_all(&groups, &incoming_url, event_sink)?;
            Ok(json!(pids))
        }
        "stop_stream" => {
            let group_id: String = get_arg(&payload, "groupId")?;
            state.ffmpeg_handler.stop(&group_id)?;
            Ok(Value::Null)
        }
        "stop_all_streams" => {
            state.ffmpeg_handler.stop_all()?;
            Ok(Value::Null)
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

        // ====================================================================
        // Device Discovery Commands (async with caching)
        // ====================================================================

        "list_cameras" => {
            let discovery = DeviceDiscovery::new(state.ffmpeg_handler.get_ffmpeg_path());
            let cameras = discovery.list_cameras_async().await?;
            Ok(json!(cameras))
        }
        "list_displays" => {
            let discovery = DeviceDiscovery::new(state.ffmpeg_handler.get_ffmpeg_path());
            let displays = discovery.list_displays_async().await?;
            Ok(json!(displays))
        }
        "list_audio_devices" => {
            // Use cpal for audio device enumeration (not FFmpeg) to ensure device IDs
            // match what cpal uses when starting capture
            let cpal_devices = state.audio_capture.list_input_devices();
            let devices: Vec<AudioInputDevice> = cpal_devices.into_iter().map(|d| {
                AudioInputDevice {
                    // Use hardware UID as the stable identifier
                    // Format: "HostId:DeviceUID" e.g. "CoreAudio:BuiltInMicrophoneDevice"
                    device_id: d.id.clone(),
                    name: d.name,
                    channels: d.channels.first().copied().unwrap_or(2) as u8,
                    sample_rate: d.sample_rates.first().copied().unwrap_or(48000),
                    is_default: d.is_default,
                }
            }).collect();
            log::info!("[list_audio_devices] Found {} devices via cpal:",
                devices.len()
            );
            for dev in &devices {
                log::info!("  - '{}' (id: {})", dev.name, dev.device_id);
            }
            Ok(json!(devices))
        }
        "list_capture_cards" => {
            let discovery = DeviceDiscovery::new(state.ffmpeg_handler.get_ffmpeg_path());
            let cards = discovery.list_capture_cards_async().await?;
            Ok(json!(cards))
        }
        "list_windows" => {
            // Use ScreenCaptureService for window enumeration (ScreenCaptureKit on macOS)
            let windows = ScreenCaptureService::list_windows_async().await;
            Ok(json!(windows))
        }
        "refresh_devices" => {
            // Return all device types at once using async parallel enumeration
            let discovery = DeviceDiscovery::new(state.ffmpeg_handler.get_ffmpeg_path());

            // Run device discovery and window enumeration in parallel
            let (all_devices, windows) = tokio::join!(
                discovery.refresh_devices_async(),
                ScreenCaptureService::list_windows_async()
            );

            let all_devices = all_devices?;

            // Use cpal for audio device enumeration (not FFmpeg) to ensure device IDs
            // match what cpal uses when starting capture
            let cpal_devices = state.audio_capture.list_input_devices();
            let audio_devices: Vec<AudioInputDevice> = cpal_devices.into_iter().map(|d| {
                AudioInputDevice {
                    // Use hardware UID as the stable identifier
                    device_id: d.id.clone(),
                    name: d.name,
                    channels: d.channels.first().copied().unwrap_or(2) as u8,
                    sample_rate: d.sample_rates.first().copied().unwrap_or(48000),
                    is_default: d.is_default,
                }
            }).collect();

            Ok(json!({
                "cameras": all_devices.cameras,
                "displays": all_devices.displays,
                "windows": windows,
                "audioDevices": audio_devices,
                "captureCards": all_devices.capture_cards
            }))
        }

        // ====================================================================
        // Source Management Commands
        // ====================================================================

        "add_source" => {
            let profile_name: String = get_arg(&payload, "profileName")?;
            let source: Source = get_arg(&payload, "source")?;
            let password: Option<String> = get_opt_arg(&payload, "password")?;

            let mut profile = state
                .profile_manager
                .load_with_key_decryption(&profile_name, password.as_deref())
                .await?;

            // Check for duplicate source ID
            if profile.sources.iter().any(|s| s.id() == source.id()) {
                return Err(format!("Source with ID {} already exists", source.id()));
            }

            profile.sources.push(source);

            let settings = state.settings_manager.load()?;
            state
                .profile_manager
                .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
                .await?;

            state.event_bus.emit("source_added", json!({ "profileName": profile_name }));
            Ok(json!(profile.sources))
        }
        "update_source" => {
            let profile_name: String = get_arg(&payload, "profileName")?;
            let source_id: String = get_arg(&payload, "sourceId")?;
            let updates: Value = get_arg(&payload, "updates")?;
            let password: Option<String> = get_opt_arg(&payload, "password")?;

            let mut profile = state
                .profile_manager
                .load_with_key_decryption(&profile_name, password.as_deref())
                .await?;

            // Find and update the source
            let source_idx = profile.sources.iter().position(|s| s.id() == source_id)
                .ok_or_else(|| format!("Source {} not found", source_id))?;

            // Check if source had audio before update
            let had_audio_before = profile.sources[source_idx].has_audio();

            // Merge updates into existing source
            let mut source_json = serde_json::to_value(&profile.sources[source_idx])
                .map_err(|e| e.to_string())?;
            if let (Some(obj), Some(upd)) = (source_json.as_object_mut(), updates.as_object()) {
                for (k, v) in upd {
                    obj.insert(k.clone(), v.clone());
                }
            }
            profile.sources[source_idx] = serde_json::from_value(source_json)
                .map_err(|e| format!("Failed to update source: {}", e))?;

            // Check if source has audio after update
            let has_audio_after = profile.sources[source_idx].has_audio();

            // Sync audio tracks in scenes if audio capability changed
            if had_audio_before != has_audio_after {
                for scene in &mut profile.scenes {
                    // Check if this source is used in the scene
                    let source_in_scene = scene.layers.iter().any(|l| l.source_id == source_id);
                    if !source_in_scene {
                        continue;
                    }

                    if has_audio_after {
                        // Add audio track if not already present
                        if !scene.audio_mixer.tracks.iter().any(|t| t.source_id == source_id) {
                            log::info!("Adding audio track for source {} to scene {} (audio enabled)", source_id, scene.name);
                            scene.audio_mixer.tracks.push(AudioTrack {
                                source_id: source_id.clone(),
                                volume: 1.0,
                                muted: false,
                                solo: false,
                            });
                        }
                    } else {
                        // Remove audio track
                        let track_count = scene.audio_mixer.tracks.len();
                        scene.audio_mixer.tracks.retain(|t| t.source_id != source_id);
                        if scene.audio_mixer.tracks.len() < track_count {
                            log::info!("Removed audio track for source {} from scene {} (audio disabled)", source_id, scene.name);
                        }
                    }
                }
            }

            let settings = state.settings_manager.load()?;
            state
                .profile_manager
                .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
                .await?;

            state.event_bus.emit("source_updated", json!({ "profileName": profile_name, "sourceId": source_id }));
            Ok(json!(profile.sources[source_idx]))
        }
        "remove_source" => {
            let profile_name: String = get_arg(&payload, "profileName")?;
            let source_id: String = get_arg(&payload, "sourceId")?;
            let password: Option<String> = get_opt_arg(&payload, "password")?;
            // If true, also remove linked audio sources. If false and linked sources exist,
            // return a confirmation request instead of deleting.
            let remove_linked: Option<bool> = get_opt_arg(&payload, "removeLinked")?;

            let mut profile = state
                .profile_manager
                .load_with_key_decryption(&profile_name, password.as_deref())
                .await?;

            // Find the source first (before any deletion)
            if !profile.sources.iter().any(|s| s.id() == source_id) {
                return Err(format!("Source {} not found", source_id));
            }

            // Find any linked audio sources
            // (AudioDeviceSources that have linked_to_source_id pointing to this source)
            let linked_audio_ids: Vec<String> = profile.sources.iter()
                .filter_map(|s| {
                    if let Source::AudioDevice(ad) = s {
                        if ad.linked_to_source_id.as_ref() == Some(&source_id) {
                            return Some(ad.id.clone());
                        }
                    }
                    None
                })
                .collect();

            // If linked sources exist and removeLinked is explicitly false, return confirmation request
            if !linked_audio_ids.is_empty() && remove_linked == Some(false) {
                // Get linked source names for the confirmation dialog
                let linked_names: Vec<String> = profile.sources.iter()
                    .filter(|s| linked_audio_ids.contains(&s.id().to_string()))
                    .map(|s| s.name().to_string())
                    .collect();

                return Ok(json!({
                    "requiresConfirmation": true,
                    "linkedSourceIds": linked_audio_ids,
                    "linkedSourceNames": linked_names,
                    "message": "This source has linked audio sources. Remove both?"
                }));
            }

            // Proceed with deletion
            profile.sources.retain(|s| s.id() != source_id);

            // Stop any running preview for this source
            state.preview_handler.stop_source_preview(&source_id);

            // Stop audio capture if running for this source
            let _ = state.audio_capture.stop_capture_for_source(&source_id);

            // Stop FFmpeg audio level extraction if running for this source
            let _ = state.audio_level_extractor.stop_extraction(&source_id);

            // Stop ScreenCaptureKit audio capture if running (macOS only)
            #[cfg(target_os = "macos")]
            let _ = state.sck_audio_capture.stop_capture(&source_id);

            // Determine if we should delete linked sources
            // Default to true (backward compatible) unless explicitly set to false
            let should_remove_linked = remove_linked.unwrap_or(true);

            // Delete linked audio sources if requested
            let removed_linked_ids: Vec<String> = if should_remove_linked && !linked_audio_ids.is_empty() {
                profile.sources.retain(|s| !linked_audio_ids.contains(&s.id().to_string()));

                // Stop previews and audio captures for linked audio sources
                for linked_id in &linked_audio_ids {
                    state.preview_handler.stop_source_preview(linked_id);
                    // Also stop audio capture and extraction for linked sources
                    let _ = state.audio_capture.stop_capture_for_source(linked_id);
                    let _ = state.audio_level_extractor.stop_extraction(linked_id);
                    // Stop ScreenCaptureKit audio capture (macOS only)
                    #[cfg(target_os = "macos")]
                    let _ = state.sck_audio_capture.stop_capture(linked_id);
                }
                linked_audio_ids.clone()
            } else {
                Vec::new()
            };

            // Also remove from all scenes
            let ids_to_remove: Vec<&String> = std::iter::once(&source_id)
                .chain(removed_linked_ids.iter())
                .collect();

            for scene in &mut profile.scenes {
                scene.layers.retain(|l| !ids_to_remove.contains(&&l.source_id));
                scene.audio_mixer.tracks.retain(|t| !ids_to_remove.contains(&&t.source_id));
            }

            let settings = state.settings_manager.load()?;
            state
                .profile_manager
                .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
                .await?;

            state.event_bus.emit("source_removed", json!({
                "profileName": profile_name,
                "sourceId": source_id,
                "linkedAudioSourceIds": removed_linked_ids
            }));

            Ok(json!({
                "removed": true,
                "linkedRemoved": removed_linked_ids
            }))
        }
        "reorder_sources" => {
            let profile_name: String = get_arg(&payload, "profileName")?;
            let source_ids: Vec<String> = get_arg(&payload, "sourceIds")?;
            let password: Option<String> = get_opt_arg(&payload, "password")?;

            let mut profile = state
                .profile_manager
                .load_with_key_decryption(&profile_name, password.as_deref())
                .await?;

            // Validate that all IDs exist
            for id in &source_ids {
                if !profile.sources.iter().any(|s| s.id() == *id) {
                    return Err(format!("Source {} not found", id));
                }
            }

            // Reorder sources based on the new order
            let mut reordered: Vec<Source> = Vec::with_capacity(source_ids.len());
            for id in &source_ids {
                if let Some(source) = profile.sources.iter().find(|s| s.id() == *id) {
                    reordered.push(source.clone());
                }
            }
            // Add any sources not in the list at the end (safety measure)
            for source in &profile.sources {
                if !source_ids.iter().any(|id| id == source.id()) {
                    reordered.push(source.clone());
                }
            }
            profile.sources = reordered;

            let settings = state.settings_manager.load()?;
            state
                .profile_manager
                .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
                .await?;

            state.event_bus.emit("sources_reordered", json!({ "profileName": profile_name }));
            Ok(json!(profile.sources))
        }

        // ====================================================================
        // Scene Management Commands
        // ====================================================================

        "create_scene" => {
            let profile_name: String = get_arg(&payload, "profileName")?;
            let name: String = get_arg(&payload, "name")?;
            let width: Option<u32> = get_opt_arg(&payload, "width")?;
            let height: Option<u32> = get_opt_arg(&payload, "height")?;
            let password: Option<String> = get_opt_arg(&payload, "password")?;

            let mut profile = state
                .profile_manager
                .load_with_key_decryption(&profile_name, password.as_deref())
                .await?;

            let scene = Scene {
                id: uuid::Uuid::new_v4().to_string(),
                name,
                canvas_width: width.unwrap_or(1920),
                canvas_height: height.unwrap_or(1080),
                layers: Vec::new(),
                audio_mixer: Default::default(),
                transition_in: None,
            };

            let scene_id = scene.id.clone();
            profile.scenes.push(scene);

            // Set as active if it's the first scene
            if profile.active_scene_id.is_none() {
                profile.active_scene_id = Some(scene_id.clone());
            }

            let settings = state.settings_manager.load()?;
            state
                .profile_manager
                .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
                .await?;

            state.event_bus.emit("scene_created", json!({ "profileName": profile_name, "sceneId": scene_id }));
            Ok(json!({ "sceneId": scene_id }))
        }
        "update_scene" => {
            let profile_name: String = get_arg(&payload, "profileName")?;
            let scene_id: String = get_arg(&payload, "sceneId")?;
            let updates: Value = get_arg(&payload, "updates")?;
            let password: Option<String> = get_opt_arg(&payload, "password")?;

            let mut profile = state
                .profile_manager
                .load_with_key_decryption(&profile_name, password.as_deref())
                .await?;

            let scene_idx = profile.scenes.iter().position(|s| s.id == scene_id)
                .ok_or_else(|| format!("Scene {} not found", scene_id))?;

            // Merge updates into existing scene
            let mut scene_json = serde_json::to_value(&profile.scenes[scene_idx])
                .map_err(|e| e.to_string())?;
            if let (Some(obj), Some(upd)) = (scene_json.as_object_mut(), updates.as_object()) {
                for (k, v) in upd {
                    obj.insert(k.clone(), v.clone());
                }
            }
            profile.scenes[scene_idx] = serde_json::from_value(scene_json)
                .map_err(|e| format!("Failed to update scene: {}", e))?;

            let settings = state.settings_manager.load()?;
            state
                .profile_manager
                .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
                .await?;

            state.event_bus.emit("scene_updated", json!({ "profileName": profile_name, "sceneId": scene_id }));
            Ok(json!(profile.scenes[scene_idx]))
        }
        "delete_scene" => {
            let profile_name: String = get_arg(&payload, "profileName")?;
            let scene_id: String = get_arg(&payload, "sceneId")?;
            let password: Option<String> = get_opt_arg(&payload, "password")?;

            let mut profile = state
                .profile_manager
                .load_with_key_decryption(&profile_name, password.as_deref())
                .await?;

            let initial_len = profile.scenes.len();
            profile.scenes.retain(|s| s.id != scene_id);

            if profile.scenes.len() == initial_len {
                return Err(format!("Scene {} not found", scene_id));
            }

            // Update active scene if deleted
            if profile.active_scene_id.as_ref() == Some(&scene_id) {
                profile.active_scene_id = profile.scenes.first().map(|s| s.id.clone());
            }

            let settings = state.settings_manager.load()?;
            state
                .profile_manager
                .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
                .await?;

            state.event_bus.emit("scene_deleted", json!({ "profileName": profile_name, "sceneId": scene_id }));
            Ok(Value::Null)
        }
        "set_active_scene" => {
            let profile_name: String = get_arg(&payload, "profileName")?;
            let scene_id: String = get_arg(&payload, "sceneId")?;
            let password: Option<String> = get_opt_arg(&payload, "password")?;

            let mut profile = state
                .profile_manager
                .load_with_key_decryption(&profile_name, password.as_deref())
                .await?;

            // Verify scene exists
            if !profile.scenes.iter().any(|s| s.id == scene_id) {
                return Err(format!("Scene {} not found", scene_id));
            }

            profile.active_scene_id = Some(scene_id.clone());

            let settings = state.settings_manager.load()?;
            state
                .profile_manager
                .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
                .await?;

            state.event_bus.emit("active_scene_changed", json!({ "profileName": profile_name, "sceneId": scene_id }));
            Ok(Value::Null)
        }
        "duplicate_scene" => {
            let profile_name: String = get_arg(&payload, "profileName")?;
            let scene_id: String = get_arg(&payload, "sceneId")?;
            let new_name: Option<String> = get_opt_arg(&payload, "newName")?;
            let password: Option<String> = get_opt_arg(&payload, "password")?;

            let mut profile = state
                .profile_manager
                .load_with_key_decryption(&profile_name, password.as_deref())
                .await?;

            let original = profile.scenes.iter().find(|s| s.id == scene_id)
                .ok_or_else(|| format!("Scene {} not found", scene_id))?
                .clone();

            let mut new_scene = original;
            new_scene.id = uuid::Uuid::new_v4().to_string();
            new_scene.name = new_name.unwrap_or_else(|| format!("{} (Copy)", new_scene.name));

            // Generate new layer IDs
            for layer in &mut new_scene.layers {
                layer.id = uuid::Uuid::new_v4().to_string();
            }

            let new_scene_id = new_scene.id.clone();
            profile.scenes.push(new_scene);

            let settings = state.settings_manager.load()?;
            state
                .profile_manager
                .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
                .await?;

            state.event_bus.emit("scene_duplicated", json!({ "profileName": profile_name, "sceneId": new_scene_id }));
            Ok(json!({ "sceneId": new_scene_id }))
        }

        // ====================================================================
        // Layer Management Commands
        // ====================================================================

        "add_layer_to_scene" => {
            let profile_name: String = get_arg(&payload, "profileName")?;
            let scene_id: String = get_arg(&payload, "sceneId")?;
            let source_id: String = get_arg(&payload, "sourceId")?;
            let transform: Option<Transform> = get_opt_arg(&payload, "transform")?;
            let password: Option<String> = get_opt_arg(&payload, "password")?;

            let mut profile = state
                .profile_manager
                .load_with_key_decryption(&profile_name, password.as_deref())
                .await?;

            // Verify source exists
            if !profile.sources.iter().any(|s| s.id() == source_id) {
                return Err(format!("Source {} not found", source_id));
            }

            // Get scene index for later use
            let scene_idx = profile.scenes.iter().position(|s| s.id == scene_id)
                .ok_or_else(|| format!("Scene {} not found", scene_id))?;

            // Get canvas dimensions and max z-index
            let (canvas_width, canvas_height, max_z) = {
                let scene = &profile.scenes[scene_idx];
                let max_z = scene.layers.iter().map(|l| l.z_index).max().unwrap_or(0);
                (scene.canvas_width, scene.canvas_height, max_z)
            };

            let layer = SourceLayer {
                id: uuid::Uuid::new_v4().to_string(),
                source_id: source_id.clone(),
                visible: true,
                locked: false,
                transform: transform.unwrap_or_else(|| Transform {
                    x: 0,
                    y: 0,
                    width: canvas_width,
                    height: canvas_height,
                    rotation: 0.0,
                    crop: None,
                }),
                z_index: max_z + 1,
            };

            let layer_id = layer.id.clone();
            profile.scenes[scene_idx].layers.push(layer);

            // Check if source is a Camera with captureAudio enabled and linked audio device
            // If so, auto-create a linked AudioDeviceSource
            let mut linked_audio_source_id: Option<String> = None;
            if let Some(Source::Camera(camera)) = profile.sources.iter().find(|s| s.id() == &source_id) {
                if camera.capture_audio {
                    if let Some(ref audio_device_id) = camera.linked_audio_device_id {
                        // Check if linked audio source already exists for this camera
                        let linked_audio_exists = profile.sources.iter().any(|s| {
                            if let Source::AudioDevice(ad) = s {
                                ad.linked_to_source_id.as_ref() == Some(&source_id)
                            } else {
                                false
                            }
                        });

                        if !linked_audio_exists {
                            // Get the audio device name (use camera name + Audio as fallback)
                            let audio_device_name = format!("{} (Audio)", camera.name);

                            // Create linked AudioDeviceSource
                            let linked_audio_id = uuid::Uuid::new_v4().to_string();
                            let linked_audio = Source::AudioDevice(AudioDeviceSource {
                                id: linked_audio_id.clone(),
                                name: audio_device_name,
                                device_id: audio_device_id.clone(),
                                channels: None,
                                sample_rate: None,
                                linked_to_source_id: Some(source_id.clone()),
                            });

                            // Add to profile sources
                            profile.sources.push(linked_audio);

                            // Add audio track for the linked audio source
                            profile.scenes[scene_idx].audio_mixer.tracks.push(AudioTrack::new(&linked_audio_id));

                            linked_audio_source_id = Some(linked_audio_id);
                        }
                    }
                }
            }

            // Add audio track for original source only if it has audio output and not already in mixer
            let source = profile.sources.iter().find(|s| s.id() == &source_id);
            let source_has_audio = source.map(|s| s.has_audio()).unwrap_or(false);

            if source_has_audio && !profile.scenes[scene_idx].audio_mixer.tracks.iter().any(|t| t.source_id == source_id) {
                profile.scenes[scene_idx].audio_mixer.tracks.push(AudioTrack {
                    source_id: source_id.clone(),
                    volume: 1.0,
                    muted: false,
                    solo: false,
                });
            }

            let settings = state.settings_manager.load()?;
            state
                .profile_manager
                .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
                .await?;

            state.event_bus.emit("layer_added", json!({
                "profileName": profile_name,
                "sceneId": scene_id,
                "layerId": layer_id,
                "linkedAudioSourceId": linked_audio_source_id
            }));
            Ok(json!({ "layerId": layer_id, "linkedAudioSourceId": linked_audio_source_id }))
        }
        "update_layer" => {
            let profile_name: String = get_arg(&payload, "profileName")?;
            let scene_id: String = get_arg(&payload, "sceneId")?;
            let layer_id: String = get_arg(&payload, "layerId")?;
            let updates: Value = get_arg(&payload, "updates")?;
            let password: Option<String> = get_opt_arg(&payload, "password")?;

            let mut profile = state
                .profile_manager
                .load_with_key_decryption(&profile_name, password.as_deref())
                .await?;

            let scene_idx = profile.scenes.iter().position(|s| s.id == scene_id)
                .ok_or_else(|| format!("Scene {} not found", scene_id))?;

            let layer_idx = profile.scenes[scene_idx].layers.iter().position(|l| l.id == layer_id)
                .ok_or_else(|| format!("Layer {} not found", layer_id))?;

            // Merge updates into existing layer
            let mut layer_json = serde_json::to_value(&profile.scenes[scene_idx].layers[layer_idx])
                .map_err(|e| e.to_string())?;
            if let (Some(obj), Some(upd)) = (layer_json.as_object_mut(), updates.as_object()) {
                for (k, v) in upd {
                    obj.insert(k.clone(), v.clone());
                }
            }
            profile.scenes[scene_idx].layers[layer_idx] = serde_json::from_value(layer_json)
                .map_err(|e| format!("Failed to update layer: {}", e))?;

            let updated_layer = profile.scenes[scene_idx].layers[layer_idx].clone();

            let settings = state.settings_manager.load()?;
            state
                .profile_manager
                .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
                .await?;

            state.event_bus.emit("layer_updated", json!({ "profileName": profile_name, "sceneId": scene_id, "layerId": layer_id }));
            Ok(json!(updated_layer))
        }
        "remove_layer" => {
            let profile_name: String = get_arg(&payload, "profileName")?;
            let scene_id: String = get_arg(&payload, "sceneId")?;
            let layer_id: String = get_arg(&payload, "layerId")?;
            let password: Option<String> = get_opt_arg(&payload, "password")?;

            let mut profile = state
                .profile_manager
                .load_with_key_decryption(&profile_name, password.as_deref())
                .await?;

            let scene = profile.scenes.iter_mut().find(|s| s.id == scene_id)
                .ok_or_else(|| format!("Scene {} not found", scene_id))?;

            let initial_len = scene.layers.len();
            scene.layers.retain(|l| l.id != layer_id);

            if scene.layers.len() == initial_len {
                return Err(format!("Layer {} not found", layer_id));
            }

            let settings = state.settings_manager.load()?;
            state
                .profile_manager
                .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
                .await?;

            state.event_bus.emit("layer_removed", json!({ "profileName": profile_name, "sceneId": scene_id, "layerId": layer_id }));
            Ok(Value::Null)
        }
        "reorder_layers" => {
            let profile_name: String = get_arg(&payload, "profileName")?;
            let scene_id: String = get_arg(&payload, "sceneId")?;
            let layer_ids: Vec<String> = get_arg(&payload, "layerIds")?;
            let password: Option<String> = get_opt_arg(&payload, "password")?;

            let mut profile = state
                .profile_manager
                .load_with_key_decryption(&profile_name, password.as_deref())
                .await?;

            let scene = profile.scenes.iter_mut().find(|s| s.id == scene_id)
                .ok_or_else(|| format!("Scene {} not found", scene_id))?;

            // Assign z-index based on provided order
            for (idx, layer_id) in layer_ids.iter().enumerate() {
                if let Some(layer) = scene.layers.iter_mut().find(|l| &l.id == layer_id) {
                    layer.z_index = idx as i32;
                }
            }

            let settings = state.settings_manager.load()?;
            state
                .profile_manager
                .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
                .await?;

            state.event_bus.emit("layers_reordered", json!({ "profileName": profile_name, "sceneId": scene_id }));
            Ok(Value::Null)
        }

        // ====================================================================
        // Audio Mixer Commands
        // ====================================================================

        "set_track_volume" => {
            let profile_name: String = get_arg(&payload, "profileName")?;
            let scene_id: String = get_arg(&payload, "sceneId")?;
            let source_id: String = get_arg(&payload, "sourceId")?;
            let volume: f32 = get_arg(&payload, "volume")?;
            let password: Option<String> = get_opt_arg(&payload, "password")?;

            let mut profile = state
                .profile_manager
                .load_with_key_decryption(&profile_name, password.as_deref())
                .await?;

            let scene = profile.scenes.iter_mut().find(|s| s.id == scene_id)
                .ok_or_else(|| format!("Scene {} not found", scene_id))?;

            let track = scene.audio_mixer.tracks.iter_mut().find(|t| t.source_id == source_id)
                .ok_or_else(|| format!("Audio track for source {} not found", source_id))?;

            track.volume = volume.clamp(0.0, 2.0);

            let settings = state.settings_manager.load()?;
            state
                .profile_manager
                .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
                .await?;

            state.event_bus.emit("track_volume_changed", json!({ "profileName": profile_name, "sceneId": scene_id, "sourceId": source_id, "volume": volume }));
            Ok(Value::Null)
        }
        "set_track_muted" => {
            let profile_name: String = get_arg(&payload, "profileName")?;
            let scene_id: String = get_arg(&payload, "sceneId")?;
            let source_id: String = get_arg(&payload, "sourceId")?;
            let muted: bool = get_arg(&payload, "muted")?;
            let password: Option<String> = get_opt_arg(&payload, "password")?;

            let mut profile = state
                .profile_manager
                .load_with_key_decryption(&profile_name, password.as_deref())
                .await?;

            let scene = profile.scenes.iter_mut().find(|s| s.id == scene_id)
                .ok_or_else(|| format!("Scene {} not found", scene_id))?;

            let track = scene.audio_mixer.tracks.iter_mut().find(|t| t.source_id == source_id)
                .ok_or_else(|| format!("Audio track for source {} not found", source_id))?;

            track.muted = muted;

            let settings = state.settings_manager.load()?;
            state
                .profile_manager
                .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
                .await?;

            state.event_bus.emit("track_muted_changed", json!({ "profileName": profile_name, "sceneId": scene_id, "sourceId": source_id, "muted": muted }));
            Ok(Value::Null)
        }
        "set_track_solo" => {
            let profile_name: String = get_arg(&payload, "profileName")?;
            let scene_id: String = get_arg(&payload, "sceneId")?;
            let source_id: String = get_arg(&payload, "sourceId")?;
            let solo: bool = get_arg(&payload, "solo")?;
            let password: Option<String> = get_opt_arg(&payload, "password")?;

            let mut profile = state
                .profile_manager
                .load_with_key_decryption(&profile_name, password.as_deref())
                .await?;

            let scene = profile.scenes.iter_mut().find(|s| s.id == scene_id)
                .ok_or_else(|| format!("Scene {} not found", scene_id))?;

            let track = scene.audio_mixer.tracks.iter_mut().find(|t| t.source_id == source_id)
                .ok_or_else(|| format!("Audio track for source {} not found", source_id))?;

            track.solo = solo;

            let settings = state.settings_manager.load()?;
            state
                .profile_manager
                .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
                .await?;

            state.event_bus.emit("track_solo_changed", json!({ "profileName": profile_name, "sceneId": scene_id, "sourceId": source_id, "solo": solo }));
            Ok(Value::Null)
        }
        "set_master_volume" => {
            let profile_name: String = get_arg(&payload, "profileName")?;
            let scene_id: String = get_arg(&payload, "sceneId")?;
            let volume: f32 = get_arg(&payload, "volume")?;
            let password: Option<String> = get_opt_arg(&payload, "password")?;

            let mut profile = state
                .profile_manager
                .load_with_key_decryption(&profile_name, password.as_deref())
                .await?;

            let scene = profile.scenes.iter_mut().find(|s| s.id == scene_id)
                .ok_or_else(|| format!("Scene {} not found", scene_id))?;

            scene.audio_mixer.master_volume = volume.clamp(0.0, 2.0);

            let settings = state.settings_manager.load()?;
            state
                .profile_manager
                .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
                .await?;

            state.event_bus.emit("master_volume_changed", json!({ "profileName": profile_name, "sceneId": scene_id, "volume": volume }));
            Ok(Value::Null)
        }
        "set_master_muted" => {
            let profile_name: String = get_arg(&payload, "profileName")?;
            let scene_id: String = get_arg(&payload, "sceneId")?;
            let muted: bool = get_arg(&payload, "muted")?;
            let password: Option<String> = get_opt_arg(&payload, "password")?;

            let mut profile = state
                .profile_manager
                .load_with_key_decryption(&profile_name, password.as_deref())
                .await?;

            let scene = profile.scenes.iter_mut().find(|s| s.id == scene_id)
                .ok_or_else(|| format!("Scene {} not found", scene_id))?;

            scene.audio_mixer.master_muted = muted;

            let settings = state.settings_manager.load()?;
            state
                .profile_manager
                .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
                .await?;

            state.event_bus.emit("master_muted_changed", json!({ "profileName": profile_name, "sceneId": scene_id, "muted": muted }));
            Ok(Value::Null)
        }

        // ====================================================================
        // Preview Commands
        // ====================================================================

        "stop_source_preview" => {
            let source_id: String = get_arg(&payload, "sourceId")?;
            state.preview_handler.stop_source_preview(&source_id);
            Ok(Value::Null)
        }
        "stop_all_previews" => {
            state.preview_handler.stop_all_previews();
            Ok(Value::Null)
        }

        // Permission commands - stubs for HTTP mode (actual permission handling is in Tauri desktop layer)
        // In HTTP/browser mode, we return granted since browser handles its own permissions
        "check_permissions" => {
            Ok(json!({
                "camera": "granted",
                "microphone": "granted",
                "screenRecording": "granted"
            }))
        }
        "get_platform" => {
            let platform = if cfg!(target_os = "macos") {
                "macos"
            } else if cfg!(target_os = "windows") {
                "windows"
            } else if cfg!(target_os = "linux") {
                "linux"
            } else {
                "unknown"
            };
            Ok(json!(platform))
        }
        "request_permission" => {
            // In HTTP mode, we can't trigger OS permission dialogs
            // Return true to indicate the request was "successful" (browser handles its own)
            Ok(json!(true))
        }
        "get_permission_guidance" => {
            let perm_type: String = get_opt_arg(&payload, "permType")?.unwrap_or_default();
            let guidance = match perm_type.as_str() {
                "camera" => "Please allow camera access in your browser when prompted.",
                "microphone" => "Please allow microphone access in your browser when prompted.",
                "screen_recording" => "Please allow screen sharing in your browser when prompted.",
                _ => "Please check your browser permissions settings.",
            };
            Ok(json!(guidance))
        }

        // Audio level monitoring commands
        "set_audio_monitor_sources" => {
            let source_ids: Vec<String> = get_arg(&payload, "sourceIds")?;
            let profile_name: Option<String> = get_opt_arg(&payload, "profileName")?;

            log::info!("=== set_audio_monitor_sources START ===");
            log::info!("Sources requested: {:?}, Profile: {:?}", source_ids, profile_name);

            // Stop extractions/captures for sources no longer in the list
            // Use active_source_ids() which returns source UUIDs (not device names)
            // This fixes the bug where device names were compared against source UUIDs
            let active_cpal_sources: Vec<String> = state.audio_capture.active_source_ids();
            for source_id in &active_cpal_sources {
                if !source_ids.contains(source_id) {
                    log::info!("Stopping cpal audio capture for removed source: {}", source_id);
                    let _ = state.audio_capture.stop_capture_for_source(source_id);
                }
            }

            // Stop FFmpeg-based extractions for sources no longer in the list
            let active_extractions: Vec<String> = state.audio_level_extractor.active_extraction_ids();
            for extraction_id in &active_extractions {
                if !source_ids.contains(extraction_id) {
                    log::info!("Stopping FFmpeg audio extraction for removed source: {}", extraction_id);
                    let _ = state.audio_level_extractor.stop_extraction(extraction_id);
                }
            }

            // Stop ScreenCaptureKit captures for sources no longer in the list (macOS only)
            #[cfg(target_os = "macos")]
            {
                let active_sck_captures: Vec<String> = state.sck_audio_capture.active_capture_ids();
                for capture_id in &active_sck_captures {
                    if !source_ids.contains(capture_id) {
                        log::info!("Stopping ScreenCaptureKit audio capture for removed source: {}", capture_id);
                        let _ = state.sck_audio_capture.stop_capture(capture_id);
                    }
                }
            }

            // Set tracked sources in the level service
            state.audio_level_service.set_tracked_sources(source_ids.clone()).await;

            // Track capture results for each source to return to frontend
            let mut capture_results: HashMap<String, serde_json::Value> = HashMap::new();

            // If profile name provided, start real audio capture for audio device sources
            if let Some(profile_name) = profile_name {
                log::info!("Loading profile '{}' to start audio capture for {} sources", profile_name, source_ids.len());
                match state.profile_manager.load(&profile_name, None).await {
                    Ok(profile) => {
                        for source_id in &source_ids {
                            // Find the source in the profile
                            if let Some(source) = profile.sources.iter().find(|s| s.id() == source_id) {
                                let source_type = match source {
                                    Source::AudioDevice(_) => "AudioDevice",
                                    Source::Rtmp(_) => "Rtmp",
                                    Source::Camera(_) => "Camera",
                                    Source::ScreenCapture(_) => "ScreenCapture",
                                    Source::MediaFile(_) => "MediaFile",
                                    _ => "Other",
                                };
                                log::info!("Found source '{}' (name: '{}') of type: {}", source_id, source.name(), source_type);

                                // Determine device to capture based on source type
                                // Only AudioDevice sources have direct device capture
                                // Other sources (ScreenCapture, MediaFile, RTMP) have embedded audio
                                // that would need FFmpeg-based level extraction
                                let capture_device: Option<(String, String)> = match source {
                                    Source::AudioDevice(audio_source) => {
                                        log::info!("Source '{}' is AudioDevice with device_id: '{}', name: '{}'",
                                            source_id, audio_source.device_id, audio_source.name);
                                        Some((audio_source.device_id.clone(), audio_source.name.clone()))
                                    }
                                    Source::ScreenCapture(screen_source) if screen_source.capture_audio => {
                                        // Screen audio capture using platform-specific methods
                                        let display_index = screen_source.display_id.parse::<u32>().unwrap_or(0);

                                        #[cfg(target_os = "macos")]
                                        {
                                            // macOS: Use ScreenCaptureKit for system audio capture
                                            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                                            let source_id_extract = source_id.clone();
                                            let audio_level_service_extract = state.audio_level_service.clone();

                                            match state.sck_audio_capture.start_display_audio_capture(
                                                &source_id,
                                                display_index,
                                                tx,
                                            ) {
                                                Ok(()) => {
                                                    tokio::spawn(async move {
                                                        while let Some(level) = rx.recv().await {
                                                            audio_level_service_extract.update_source_level(
                                                                &source_id_extract,
                                                                level.left_rms.unwrap_or(level.rms),
                                                                level.right_rms.unwrap_or(level.rms),
                                                                level.left_peak.unwrap_or(level.peak),
                                                                level.right_peak.unwrap_or(level.peak),
                                                            ).await;
                                                        }
                                                    });
                                                    capture_results.insert(source_id.clone(), json!({
                                                        "success": true,
                                                        "sourceType": "ScreenCapture",
                                                        "displayIndex": display_index,
                                                        "method": "ScreenCaptureKit"
                                                    }));
                                                    log::info!("✓ Audio capture STARTED for ScreenCapture source '{}' via ScreenCaptureKit", source_id);
                                                }
                                                Err(e) => {
                                                    capture_results.insert(source_id.clone(), json!({
                                                        "success": false,
                                                        "sourceType": "ScreenCapture",
                                                        "reason": "captureError",
                                                        "message": format!("Screen audio capture failed: {}. Make sure screen recording permission is granted.", e)
                                                    }));
                                                    log::warn!("✗ ScreenCaptureKit audio capture FAILED for source '{}': {}", source_id, e);
                                                }
                                            }
                                        }

                                        #[cfg(not(target_os = "macos"))]
                                        {
                                            // Windows/Linux: Use FFmpeg-based system audio capture
                                            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                                            let source_id_extract = source_id.clone();
                                            let audio_level_service_extract = state.audio_level_service.clone();

                                            match state.audio_level_extractor.start_screen_capture_extraction(
                                                &source_id,
                                                display_index,
                                                tx,
                                            ) {
                                                Ok(()) => {
                                                    tokio::spawn(async move {
                                                        while let Some(level) = rx.recv().await {
                                                            audio_level_service_extract.update_source_level(
                                                                &source_id_extract,
                                                                level.left_rms.unwrap_or(level.rms),
                                                                level.right_rms.unwrap_or(level.rms),
                                                                level.left_peak.unwrap_or(level.peak),
                                                                level.right_peak.unwrap_or(level.peak),
                                                            ).await;
                                                        }
                                                    });
                                                    capture_results.insert(source_id.clone(), json!({
                                                        "success": true,
                                                        "sourceType": "ScreenCapture",
                                                        "displayIndex": display_index
                                                    }));
                                                    log::info!("✓ Audio extraction STARTED for ScreenCapture source '{}'", source_id);
                                                }
                                                Err(e) => {
                                                    capture_results.insert(source_id.clone(), json!({
                                                        "success": false,
                                                        "sourceType": "ScreenCapture",
                                                        "reason": "extractionFailed",
                                                        "message": format!("Screen audio metering unavailable: {}", e)
                                                    }));
                                                    log::warn!("✗ Audio extraction FAILED for ScreenCapture source '{}': {}", source_id, e);
                                                }
                                            }
                                        }
                                        None
                                    }
                                    Source::MediaFile(media_source) => {
                                        // Start FFmpeg-based audio level extraction for media file
                                        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                                        let source_id_extract = source_id.clone();
                                        let audio_level_service_extract = state.audio_level_service.clone();
                                        let file_path = media_source.file_path.clone();

                                        match state.audio_level_extractor.start_media_file_extraction(
                                            &source_id,
                                            &file_path,
                                            tx,
                                        ) {
                                            Ok(()) => {
                                                // Spawn task to route extracted levels to AudioLevelService
                                                tokio::spawn(async move {
                                                    while let Some(level) = rx.recv().await {
                                                        audio_level_service_extract.update_source_level(
                                                            &source_id_extract,
                                                            level.left_rms.unwrap_or(level.rms),
                                                            level.right_rms.unwrap_or(level.rms),
                                                            level.left_peak.unwrap_or(level.peak),
                                                            level.right_peak.unwrap_or(level.peak),
                                                        ).await;
                                                    }
                                                });
                                                capture_results.insert(source_id.clone(), json!({
                                                    "success": true,
                                                    "sourceType": "MediaFile",
                                                    "filePath": file_path
                                                }));
                                                log::info!("✓ Audio extraction STARTED for MediaFile source '{}'", source_id);
                                            }
                                            Err(e) => {
                                                // Check if error is about unsupported format
                                                let reason = if e.contains("not a supported media format") {
                                                    "unsupportedFormat"
                                                } else {
                                                    "extractionFailed"
                                                };
                                                capture_results.insert(source_id.clone(), json!({
                                                    "success": false,
                                                    "sourceType": "MediaFile",
                                                    "reason": reason,
                                                    "message": format!("Failed to start audio extraction: {}", e)
                                                }));
                                                log::warn!("✗ Audio extraction FAILED for MediaFile source '{}': {}", source_id, e);
                                            }
                                        }
                                        None
                                    }
                                    Source::Rtmp(rtmp_source) => {
                                        // Start FFmpeg-based audio level extraction for RTMP stream
                                        let rtmp_url = format!(
                                            "rtmp://{}:{}/{}",
                                            rtmp_source.bind_address,
                                            rtmp_source.port,
                                            rtmp_source.application
                                        );
                                        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                                        let source_id_extract = source_id.clone();
                                        let audio_level_service_extract = state.audio_level_service.clone();

                                        match state.audio_level_extractor.start_rtmp_extraction(
                                            &source_id,
                                            &rtmp_url,
                                            tx,
                                        ) {
                                            Ok(()) => {
                                                // Spawn task to route extracted levels to AudioLevelService
                                                tokio::spawn(async move {
                                                    while let Some(level) = rx.recv().await {
                                                        audio_level_service_extract.update_source_level(
                                                            &source_id_extract,
                                                            level.left_rms.unwrap_or(level.rms),
                                                            level.right_rms.unwrap_or(level.rms),
                                                            level.left_peak.unwrap_or(level.peak),
                                                            level.right_peak.unwrap_or(level.peak),
                                                        ).await;
                                                    }
                                                });
                                                capture_results.insert(source_id.clone(), json!({
                                                    "success": true,
                                                    "sourceType": "Rtmp",
                                                    "rtmpUrl": rtmp_url
                                                }));
                                                log::info!("✓ Audio extraction STARTED for RTMP source '{}'", source_id);
                                            }
                                            Err(e) => {
                                                capture_results.insert(source_id.clone(), json!({
                                                    "success": false,
                                                    "sourceType": "Rtmp",
                                                    "reason": "extractionFailed",
                                                    "message": format!("Failed to start audio extraction: {}", e)
                                                }));
                                                log::warn!("✗ Audio extraction FAILED for RTMP source '{}': {}", source_id, e);
                                            }
                                        }
                                        None
                                    }
                                    Source::CaptureCard(card_source) if card_source.capture_audio => {
                                        // Start FFmpeg-based audio level extraction for capture card
                                        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                                        let source_id_extract = source_id.clone();
                                        let audio_level_service_extract = state.audio_level_service.clone();
                                        let device_id = card_source.device_id.clone();

                                        match state.audio_level_extractor.start_capture_card_extraction(
                                            &source_id,
                                            &device_id,
                                            tx,
                                        ) {
                                            Ok(()) => {
                                                // Spawn task to route extracted levels to AudioLevelService
                                                tokio::spawn(async move {
                                                    while let Some(level) = rx.recv().await {
                                                        audio_level_service_extract.update_source_level(
                                                            &source_id_extract,
                                                            level.left_rms.unwrap_or(level.rms),
                                                            level.right_rms.unwrap_or(level.rms),
                                                            level.left_peak.unwrap_or(level.peak),
                                                            level.right_peak.unwrap_or(level.peak),
                                                        ).await;
                                                    }
                                                });
                                                capture_results.insert(source_id.clone(), json!({
                                                    "success": true,
                                                    "sourceType": "CaptureCard",
                                                    "deviceId": device_id
                                                }));
                                                log::info!("✓ Audio extraction STARTED for CaptureCard source '{}'", source_id);
                                            }
                                            Err(e) => {
                                                capture_results.insert(source_id.clone(), json!({
                                                    "success": false,
                                                    "sourceType": "CaptureCard",
                                                    "reason": "extractionFailed",
                                                    "message": format!("Failed to start audio extraction: {}", e)
                                                }));
                                                log::warn!("✗ Audio extraction FAILED for CaptureCard source '{}': {}", source_id, e);
                                            }
                                        }
                                        None
                                    }
                                    Source::WindowCapture(win_source) if win_source.capture_audio => {
                                        // Window audio capture using platform-specific methods
                                        let window_id = win_source.window_id.clone();

                                        #[cfg(target_os = "macos")]
                                        {
                                            // macOS: Use ScreenCaptureKit for window audio capture
                                            let window_id_num = window_id.parse::<u32>().unwrap_or(0);
                                            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                                            let source_id_extract = source_id.clone();
                                            let audio_level_service_extract = state.audio_level_service.clone();

                                            match state.sck_audio_capture.start_window_audio_capture(
                                                &source_id,
                                                window_id_num,
                                                tx,
                                            ) {
                                                Ok(()) => {
                                                    tokio::spawn(async move {
                                                        while let Some(level) = rx.recv().await {
                                                            audio_level_service_extract.update_source_level(
                                                                &source_id_extract,
                                                                level.left_rms.unwrap_or(level.rms),
                                                                level.right_rms.unwrap_or(level.rms),
                                                                level.left_peak.unwrap_or(level.peak),
                                                                level.right_peak.unwrap_or(level.peak),
                                                            ).await;
                                                        }
                                                    });
                                                    capture_results.insert(source_id.clone(), json!({
                                                        "success": true,
                                                        "sourceType": "WindowCapture",
                                                        "windowId": window_id,
                                                        "method": "ScreenCaptureKit"
                                                    }));
                                                    log::info!("✓ Audio capture STARTED for WindowCapture source '{}' via ScreenCaptureKit", source_id);
                                                }
                                                Err(e) => {
                                                    capture_results.insert(source_id.clone(), json!({
                                                        "success": false,
                                                        "sourceType": "WindowCapture",
                                                        "reason": "captureError",
                                                        "message": format!("Window audio capture failed: {}. Make sure screen recording permission is granted.", e)
                                                    }));
                                                    log::warn!("✗ ScreenCaptureKit audio capture FAILED for WindowCapture source '{}': {}", source_id, e);
                                                }
                                            }
                                        }

                                        #[cfg(not(target_os = "macos"))]
                                        {
                                            // Windows/Linux: Use FFmpeg-based system audio capture
                                            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                                            let source_id_extract = source_id.clone();
                                            let audio_level_service_extract = state.audio_level_service.clone();

                                            match state.audio_level_extractor.start_window_capture_extraction(
                                                &source_id,
                                                &window_id,
                                                tx,
                                            ) {
                                                Ok(()) => {
                                                    tokio::spawn(async move {
                                                        while let Some(level) = rx.recv().await {
                                                            audio_level_service_extract.update_source_level(
                                                                &source_id_extract,
                                                                level.left_rms.unwrap_or(level.rms),
                                                                level.right_rms.unwrap_or(level.rms),
                                                                level.left_peak.unwrap_or(level.peak),
                                                                level.right_peak.unwrap_or(level.peak),
                                                            ).await;
                                                        }
                                                    });
                                                    capture_results.insert(source_id.clone(), json!({
                                                        "success": true,
                                                        "sourceType": "WindowCapture",
                                                        "windowId": window_id
                                                    }));
                                                    log::info!("✓ Audio extraction STARTED for WindowCapture source '{}'", source_id);
                                                }
                                                Err(e) => {
                                                    capture_results.insert(source_id.clone(), json!({
                                                        "success": false,
                                                        "sourceType": "WindowCapture",
                                                        "reason": "extractionFailed",
                                                        "message": format!("Failed to start audio extraction: {}", e)
                                                    }));
                                                    log::warn!("✗ Audio extraction FAILED for WindowCapture source '{}': {}", source_id, e);
                                                }
                                            }
                                        }
                                        None
                                    }
                                    Source::GameCapture(game_source) if game_source.capture_audio => {
                                        // Game audio capture using platform-specific methods

                                        #[cfg(target_os = "macos")]
                                        {
                                            // macOS: Use ScreenCaptureKit for system audio capture (game audio)
                                            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                                            let source_id_extract = source_id.clone();
                                            let audio_level_service_extract = state.audio_level_service.clone();

                                            match state.sck_audio_capture.start_system_audio_capture(
                                                &source_id,
                                                tx,
                                            ) {
                                                Ok(()) => {
                                                    tokio::spawn(async move {
                                                        while let Some(level) = rx.recv().await {
                                                            audio_level_service_extract.update_source_level(
                                                                &source_id_extract,
                                                                level.left_rms.unwrap_or(level.rms),
                                                                level.right_rms.unwrap_or(level.rms),
                                                                level.left_peak.unwrap_or(level.peak),
                                                                level.right_peak.unwrap_or(level.peak),
                                                            ).await;
                                                        }
                                                    });
                                                    capture_results.insert(source_id.clone(), json!({
                                                        "success": true,
                                                        "sourceType": "GameCapture",
                                                        "method": "ScreenCaptureKit"
                                                    }));
                                                    log::info!("✓ Audio capture STARTED for GameCapture source '{}' via ScreenCaptureKit", source_id);
                                                }
                                                Err(e) => {
                                                    capture_results.insert(source_id.clone(), json!({
                                                        "success": false,
                                                        "sourceType": "GameCapture",
                                                        "reason": "captureError",
                                                        "message": format!("Game audio capture failed: {}. Make sure screen recording permission is granted.", e)
                                                    }));
                                                    log::warn!("✗ ScreenCaptureKit audio capture FAILED for GameCapture source '{}': {}", source_id, e);
                                                }
                                            }
                                        }

                                        #[cfg(not(target_os = "macos"))]
                                        {
                                            // Windows/Linux: Use FFmpeg-based system audio capture
                                            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                                            let source_id_extract = source_id.clone();
                                            let audio_level_service_extract = state.audio_level_service.clone();

                                            match state.audio_level_extractor.start_game_capture_extraction(
                                                &source_id,
                                                tx,
                                            ) {
                                                Ok(()) => {
                                                    tokio::spawn(async move {
                                                        while let Some(level) = rx.recv().await {
                                                            audio_level_service_extract.update_source_level(
                                                                &source_id_extract,
                                                                level.left_rms.unwrap_or(level.rms),
                                                                level.right_rms.unwrap_or(level.rms),
                                                                level.left_peak.unwrap_or(level.peak),
                                                                level.right_peak.unwrap_or(level.peak),
                                                            ).await;
                                                        }
                                                    });
                                                    capture_results.insert(source_id.clone(), json!({
                                                        "success": true,
                                                        "sourceType": "GameCapture"
                                                    }));
                                                    log::info!("✓ Audio extraction STARTED for GameCapture source '{}'", source_id);
                                                }
                                                Err(e) => {
                                                    capture_results.insert(source_id.clone(), json!({
                                                        "success": false,
                                                        "sourceType": "GameCapture",
                                                        "reason": "extractionFailed",
                                                        "message": format!("Failed to start audio extraction: {}", e)
                                                    }));
                                                    log::warn!("✗ Audio extraction FAILED for GameCapture source '{}': {}", source_id, e);
                                                }
                                            }
                                        }
                                        None
                                    }
                                    Source::MediaPlaylist(playlist_source) => {
                                        // Start audio extraction from current playlist item
                                        if let Some(current_item) = playlist_source.items.get(playlist_source.current_item_index) {
                                            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                                            let source_id_extract = source_id.clone();
                                            let audio_level_service_extract = state.audio_level_service.clone();
                                            let file_path = current_item.file_path.clone();

                                            match state.audio_level_extractor.start_media_playlist_extraction(
                                                &source_id,
                                                &file_path,
                                                tx,
                                            ) {
                                                Ok(()) => {
                                                    tokio::spawn(async move {
                                                        while let Some(level) = rx.recv().await {
                                                            audio_level_service_extract.update_source_level(
                                                                &source_id_extract,
                                                                level.left_rms.unwrap_or(level.rms),
                                                                level.right_rms.unwrap_or(level.rms),
                                                                level.left_peak.unwrap_or(level.peak),
                                                                level.right_peak.unwrap_or(level.peak),
                                                            ).await;
                                                        }
                                                    });
                                                    capture_results.insert(source_id.clone(), json!({
                                                        "success": true,
                                                        "sourceType": "MediaPlaylist",
                                                        "currentFile": file_path
                                                    }));
                                                    log::info!("✓ Audio extraction STARTED for MediaPlaylist source '{}'", source_id);
                                                }
                                                Err(e) => {
                                                    capture_results.insert(source_id.clone(), json!({
                                                        "success": false,
                                                        "sourceType": "MediaPlaylist",
                                                        "reason": "extractionFailed",
                                                        "message": format!("Failed to start audio extraction: {}", e)
                                                    }));
                                                    log::warn!("✗ Audio extraction FAILED for MediaPlaylist source '{}': {}", source_id, e);
                                                }
                                            }
                                        } else {
                                            capture_results.insert(source_id.clone(), json!({
                                                "success": false,
                                                "sourceType": "MediaPlaylist",
                                                "reason": "noCurrentItem",
                                                "message": "Playlist has no current item"
                                            }));
                                        }
                                        None
                                    }
                                    Source::Ndi(ndi_source) if ndi_source.capture_audio => {
                                        // Start FFmpeg-based audio level extraction for NDI
                                        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                                        let source_id_extract = source_id.clone();
                                        let audio_level_service_extract = state.audio_level_service.clone();
                                        let ndi_name = ndi_source.source_name.clone();
                                        let ip_addr = ndi_source.ip_address.clone();

                                        match state.audio_level_extractor.start_ndi_extraction(
                                            &source_id,
                                            &ndi_name,
                                            ip_addr.as_deref(),
                                            tx,
                                        ) {
                                            Ok(()) => {
                                                tokio::spawn(async move {
                                                    while let Some(level) = rx.recv().await {
                                                        audio_level_service_extract.update_source_level(
                                                            &source_id_extract,
                                                            level.left_rms.unwrap_or(level.rms),
                                                            level.right_rms.unwrap_or(level.rms),
                                                            level.left_peak.unwrap_or(level.peak),
                                                            level.right_peak.unwrap_or(level.peak),
                                                        ).await;
                                                    }
                                                });
                                                capture_results.insert(source_id.clone(), json!({
                                                    "success": true,
                                                    "sourceType": "Ndi",
                                                    "ndiSource": ndi_name
                                                }));
                                                log::info!("✓ Audio extraction STARTED for NDI source '{}'", source_id);
                                            }
                                            Err(e) => {
                                                capture_results.insert(source_id.clone(), json!({
                                                    "success": false,
                                                    "sourceType": "Ndi",
                                                    "reason": "extractionFailed",
                                                    "message": format!("Failed to start audio extraction: {}", e)
                                                }));
                                                log::warn!("✗ Audio extraction FAILED for NDI source '{}': {}", source_id, e);
                                            }
                                        }
                                        None
                                    }
                                    _ => {
                                        capture_results.insert(source_id.clone(), json!({
                                            "success": false,
                                            "sourceType": source_type,
                                            "reason": "noAudio",
                                            "message": "Source type does not support audio capture"
                                        }));
                                        None
                                    }
                                };

                                // Start audio capture if we have a device
                                if let Some((device_id, device_name)) = capture_device {
                                    let source_id_clone = source_id.clone();
                                    let audio_level_service = state.audio_level_service.clone();

                                    // Skip if already capturing for this source
                                    if state.audio_capture.is_capturing_source(source_id) {
                                        log::info!("Already capturing audio for source '{}', skipping", source_id);
                                        capture_results.insert(source_id.clone(), json!({
                                            "success": true,
                                            "sourceType": "AudioDevice",
                                            "deviceName": device_name,
                                            "alreadyCapturing": true
                                        }));
                                        continue;
                                    }

                                    // Try device NAME first (more reliable - name matching is unambiguous)
                                    // Fall back to device_id (numeric index) only if name matching fails
                                    // This avoids the FFmpeg/cpal device index mismatch problem
                                    // Use start_input_capture_for_source to track source_id -> device mapping
                                    log::info!("Attempting audio capture for source '{}': trying device name '{}' first", source_id, device_name);
                                    let capture_result = state.audio_capture.start_input_capture_for_source(
                                        source_id,
                                        &device_name,
                                        AudioCaptureConfig::default(),
                                    ).or_else(|e| {
                                        // If device name failed, try the device_id (numeric index as fallback)
                                        log::info!("Device name '{}' failed ({}), trying device_id '{}'", device_name, e, device_id);
                                        state.audio_capture.start_input_capture_for_source(
                                            source_id,
                                            &device_id,
                                            AudioCaptureConfig::default(),
                                        )
                                    });

                                    match capture_result {
                                        Ok(mut receiver) => {
                                            log::info!("✓ Audio capture STARTED for source '{}' (device: '{}')", source_id, device_name);
                                            capture_results.insert(source_id.clone(), json!({
                                                "success": true,
                                                "sourceType": "AudioDevice",
                                                "deviceName": device_name
                                            }));

                                            // Spawn task to process audio and update levels
                                            let device_name_log = device_name.clone();
                                            tokio::spawn(async move {
                                                log::info!("Audio capture task running for source {} (device {})", source_id_clone, device_name_log);
                                                let mut buffer_count = 0u64;
                                                // Use loop with match to handle Lagged errors gracefully
                                                // The old "while let Ok" would exit on ANY error including Lagged
                                                loop {
                                                    match receiver.recv().await {
                                                        Ok(buffer) => {
                                                            // Calculate separate L/R RMS and Peak for stereo sources
                                                            // Following OBS's audio metering model:
                                                            // - RMS = root mean square (average power)
                                                            // - Peak = instantaneous maximum absolute sample value
                                                            // Interleaved samples for N channels: [ch0_s0, ch1_s0, ch2_s0, ..., ch0_s1, ch1_s1, ...]
                                                            let channels = buffer.channels as usize;
                                                            let (rms_l, rms_r, peak_l, peak_r) = if buffer.samples.is_empty() || channels == 0 {
                                                                (0.0, 0.0, 0.0, 0.0)
                                                            } else if channels >= 2 {
                                                                // Stereo or multi-channel: extract channels 0 (L) and 1 (R)
                                                                // Use proper deinterleaving based on actual channel count
                                                                let mut sum_squares_l = 0.0f32;
                                                                let mut sum_squares_r = 0.0f32;
                                                                let mut max_abs_l = 0.0f32;
                                                                let mut max_abs_r = 0.0f32;
                                                                let mut count = 0usize;
                                                                for frame in buffer.samples.chunks_exact(channels) {
                                                                    let sample_l = frame[0]; // Channel 0 = Left
                                                                    let sample_r = frame[1]; // Channel 1 = Right
                                                                    sum_squares_l += sample_l * sample_l;
                                                                    sum_squares_r += sample_r * sample_r;
                                                                    max_abs_l = max_abs_l.max(sample_l.abs());
                                                                    max_abs_r = max_abs_r.max(sample_r.abs());
                                                                    count += 1;
                                                                }
                                                                let rms_l = if count > 0 { (sum_squares_l / count as f32).sqrt() } else { 0.0 };
                                                                let rms_r = if count > 0 { (sum_squares_r / count as f32).sqrt() } else { 0.0 };
                                                                (rms_l, rms_r, max_abs_l, max_abs_r)
                                                            } else {
                                                                // Mono: use same value for both channels
                                                                let mut sum_squares = 0.0f32;
                                                                let mut max_abs = 0.0f32;
                                                                for &sample in buffer.samples.iter() {
                                                                    sum_squares += sample * sample;
                                                                    max_abs = max_abs.max(sample.abs());
                                                                }
                                                                let rms = (sum_squares / buffer.samples.len() as f32).sqrt();
                                                                (rms, rms, max_abs, max_abs)
                                                            };
                                                            // Update the audio level service with stereo RMS and Peak values
                                                            audio_level_service.update_source_level(&source_id_clone, rms_l, rms_r, peak_l, peak_r).await;

                                                            // Log periodically to confirm data is flowing
                                                            buffer_count += 1;
                                                            // Log first 5 buffers, then every 100th (about every 3 seconds)
                                                            if buffer_count <= 5 || buffer_count % 100 == 0 {
                                                                log::info!("[AudioCapture] Buffer #{} for '{}': RMS L={:.4} R={:.4}, Peak L={:.4} R={:.4}, ch={}, samples={}",
                                                                    buffer_count, source_id_clone, rms_l, rms_r, peak_l, peak_r, buffer.channels, buffer.samples.len());
                                                            }
                                                        }
                                                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                                            // Missed some buffers due to slow processing - this is OK, just continue
                                                            log::debug!("Audio capture lagged {} buffers for {} (processing continues)", n, source_id_clone);
                                                            continue;
                                                        }
                                                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                                            // Channel closed, stop processing
                                                            log::info!("Audio capture channel closed for {}", source_id_clone);
                                                            break;
                                                        }
                                                    }
                                                }
                                                log::info!("Audio capture ended for source {} (processed {} buffers)", source_id_clone, buffer_count);
                                            });
                                        }
                                        Err(e) => {
                                            log::error!("✗ Audio capture FAILED for source '{}' (device_name: '{}', device_id: '{}'): {}",
                                                source_id, device_name, device_id, e);
                                            capture_results.insert(source_id.clone(), json!({
                                                "success": false,
                                                "sourceType": "AudioDevice",
                                                "deviceName": device_name,
                                                "reason": "captureFailed",
                                                "message": format!("Failed to capture from device: {}", e)
                                            }));
                                        }
                                    }
                                }
                            } else {
                                // Source not found in profile
                                capture_results.insert(source_id.clone(), json!({
                                    "success": false,
                                    "reason": "notFound",
                                    "message": "Source not found in profile"
                                }));
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to load profile '{}': {}", profile_name, e);
                        // Mark all sources as failed due to profile load error
                        for source_id in &source_ids {
                            capture_results.insert(source_id.clone(), json!({
                                "success": false,
                                "reason": "profileLoadFailed",
                                "message": format!("Failed to load profile: {}", e)
                            }));
                        }
                    }
                }
            }

            Ok(json!({
                "captureResults": capture_results,
                "trackedSources": source_ids.len()
            }))
        }
        "get_audio_monitor_status" => {
            Ok(json!({
                "running": state.audio_level_service.is_running()
            }))
        }

        "get_audio_monitor_health" => {
            let health = state.audio_level_service.get_health_status().await;
            Ok(json!({
                "sources": health
            }))
        }

        _ => Err(format!("Unknown command: {command}")),
    }
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
// Native Capture API Handlers
// ============================================================================

// --- Device Discovery ---

/// GET /api/devices/cameras - List available cameras
async fn list_cameras_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let cameras = state.camera_capture.list_cameras();
    Json(json!({ "ok": true, "data": cameras }))
}

/// GET /api/devices/displays - List available displays
/// Uses async version with spawn_blocking and timeout protection
async fn list_displays_handler() -> impl IntoResponse {
    let displays = ScreenCaptureService::list_displays_async().await;
    Json(json!({ "ok": true, "data": displays }))
}

/// GET /api/devices/audio/input - List audio input devices (microphones)
async fn list_audio_input_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let devices = state.audio_capture.list_input_devices();
    Json(json!({ "ok": true, "data": devices }))
}

/// GET /api/devices/audio/output - List audio output devices
async fn list_audio_output_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let devices = state.audio_capture.list_output_devices();
    Json(json!({ "ok": true, "data": devices }))
}

/// GET /api/devices/capture-cards - List available capture cards (Elgato, etc.)
async fn list_capture_cards_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let discovery = DeviceDiscovery::new(state.ffmpeg_handler.get_ffmpeg_path());
    match discovery.list_capture_cards_async().await {
        Ok(cards) => Json(json!({ "ok": true, "data": cards })),
        Err(e) => Json(json!({ "ok": false, "error": format!("Failed to list capture cards: {}", e) })),
    }
}

/// GET /api/devices/windows - List capturable windows
/// Uses ScreenCaptureKit on macOS for window enumeration
async fn list_windows_handler() -> impl IntoResponse {
    let windows = ScreenCaptureService::list_windows_async().await;
    Json(json!({ "ok": true, "data": windows }))
}

// --- Capture Control ---

#[derive(Debug, Deserialize)]
struct CameraCaptureRequest {
    device_id: String,
}

/// POST /api/capture/camera/start - Start camera capture
async fn start_camera_capture_handler(
    State(state): State<AppState>,
    Json(req): Json<CameraCaptureRequest>,
) -> impl IntoResponse {
    match state.camera_capture.start_capture(&req.device_id, CameraCaptureConfig::default()) {
        Ok(_) => {
            state.capture_indicator.register_capture(
                CaptureType::Camera(req.device_id.clone()),
                Some(&state.event_bus),
            );
            Json(json!({ "ok": true, "data": null }))
        }
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

/// POST /api/capture/camera/stop - Stop camera capture
async fn stop_camera_capture_handler(
    State(state): State<AppState>,
    Json(req): Json<CameraCaptureRequest>,
) -> impl IntoResponse {
    let _ = state.camera_capture.stop_capture(&req.device_id);
    state.capture_indicator.unregister_capture(
        &CaptureType::Camera(req.device_id.clone()),
        Some(&state.event_bus),
    );
    Json(json!({ "ok": true, "data": null }))
}

#[derive(Debug, Deserialize)]
struct ScreenCaptureRequest {
    display_id: String,
}

/// POST /api/capture/screen/start - Start screen capture
/// Uses spawn_blocking because scap::get_all_targets() can block/hang on macOS
async fn start_screen_capture_handler(
    State(state): State<AppState>,
    Json(req): Json<ScreenCaptureRequest>,
) -> impl IntoResponse {
    let display_id: u32 = match req.display_id.parse() {
        Ok(id) => id,
        Err(_) => return Json(json!({ "ok": false, "error": "Invalid display_id: must be a number" })),
    };

    // Clone what we need for the blocking task
    let screen_capture = state.screen_capture.clone();

    // Run in spawn_blocking because start_display_capture calls scap::get_all_targets()
    let result = tokio::task::spawn_blocking(move || {
        screen_capture.start_display_capture(display_id, ScreenCaptureConfig::default())
    }).await;

    match result {
        Ok(Ok(_)) => {
            state.capture_indicator.register_capture(
                CaptureType::Screen(req.display_id.clone()),
                Some(&state.event_bus),
            );
            Json(json!({ "ok": true, "data": null }))
        }
        Ok(Err(e)) => Json(json!({ "ok": false, "error": e })),
        Err(join_err) => Json(json!({ "ok": false, "error": format!("Task panicked: {}", join_err) })),
    }
}

/// POST /api/capture/screen/stop - Stop screen capture
async fn stop_screen_capture_handler(
    State(state): State<AppState>,
    Json(req): Json<ScreenCaptureRequest>,
) -> impl IntoResponse {
    let capture_id = format!("display_{}", req.display_id);

    let _ = state.screen_capture.stop_capture(&capture_id);
    state.capture_indicator.unregister_capture(
        &CaptureType::Screen(req.display_id.clone()),
        Some(&state.event_bus),
    );
    Json(json!({ "ok": true, "data": null }))
}

#[derive(Debug, Deserialize)]
struct AudioCaptureRequest {
    device_id: String,
    #[serde(default)]
    is_loopback: bool,
}

/// POST /api/capture/audio/start - Start audio capture
async fn start_audio_capture_handler(
    State(state): State<AppState>,
    Json(req): Json<AudioCaptureRequest>,
) -> impl IntoResponse {
    let result = if req.is_loopback {
        state.audio_capture.start_loopback_capture(&req.device_id, AudioCaptureConfig::default())
    } else {
        state.audio_capture.start_input_capture(&req.device_id, AudioCaptureConfig::default())
    };

    match result {
        Ok(_) => {
            let capture_type = if req.is_loopback {
                CaptureType::SystemAudio
            } else {
                CaptureType::Microphone(req.device_id.clone())
            };
            state.capture_indicator.register_capture(capture_type, Some(&state.event_bus));
            Json(json!({ "ok": true, "data": null }))
        }
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

/// POST /api/capture/audio/stop - Stop audio capture
async fn stop_audio_capture_handler(
    State(state): State<AppState>,
    Json(req): Json<AudioCaptureRequest>,
) -> impl IntoResponse {
    let _ = state.audio_capture.stop_capture(&req.device_id);
    let capture_type = if req.is_loopback {
        CaptureType::SystemAudio
    } else {
        CaptureType::Microphone(req.device_id.clone())
    };
    state.capture_indicator.unregister_capture(&capture_type, Some(&state.event_bus));
    Json(json!({ "ok": true, "data": null }))
}

/// GET /api/capture/status - Get capture status
async fn capture_status_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let status = state.capture_indicator.get_status();
    Json(json!({ "ok": true, "data": status }))
}

/// GET /api/capture/{source_id}/stream - Stream MPEG-TS H264 capture to go2rtc
/// This endpoint serves the H264 encoded MPEG-TS stream from our capture service.
/// go2rtc uses this with #video=copy to passthrough without re-encoding.
async fn capture_stream_handler(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> impl IntoResponse {
    // Get a subscriber to the existing stream
    let rx = match state.h264_capture.subscribe_to_stream(&source_id) {
        Some(rx) => rx,
        None => {
            log::warn!("No H264 capture stream found for source: {}", source_id);
            return (
                StatusCode::NOT_FOUND,
                [(header::CONTENT_TYPE, "text/plain")],
                axum::body::Body::from("Stream not found"),
            ).into_response();
        }
    };

    log::info!("Starting MPEG-TS stream for source: {}", source_id);

    // Create a streaming response using the broadcast receiver
    let stream = async_stream::stream! {
        let mut rx = rx;
        loop {
            match rx.recv().await {
                Ok(chunk) => {
                    yield Ok::<_, std::io::Error>(chunk);
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    log::warn!("Stream consumer lagged by {} chunks for {}", n, source_id);
                    // Continue receiving
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    log::info!("H264 capture stream closed for {}", source_id);
                    break;
                }
            }
        }
    };

    let body = axum::body::Body::from_stream(stream);

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "video/mp2t"),
            (header::CACHE_CONTROL, "no-cache, no-store"),
            (header::CONNECTION, "keep-alive"),
        ],
        body,
    ).into_response()
}

// --- Audio Levels ---

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetAudioLevelSourcesRequest {
    source_ids: Vec<String>,
    profile_name: Option<String>,
}

/// POST /api/audio-levels/start - Start audio level monitoring with source list
async fn audio_levels_start_handler(
    State(state): State<AppState>,
    Json(req): Json<SetAudioLevelSourcesRequest>,
) -> impl IntoResponse {
    log::info!(
        "[AudioLevels] Starting monitoring for {} sources",
        req.source_ids.len()
    );

    // Set tracked sources
    state
        .audio_level_service
        .set_tracked_sources(req.source_ids.clone())
        .await;

    // If profile name provided, start audio capture for device sources
    let mut capture_results = std::collections::HashMap::new();
    if let Some(profile_name) = &req.profile_name {
        // Load the profile asynchronously
        if let Ok(profile) = state.profile_manager.load(profile_name, None).await {
            for source_id in &req.source_ids {
                if let Some(source) = profile.sources.iter().find(|s| s.id() == source_id) {
                    if let Source::AudioDevice(audio_source) = source {
                        // Start audio capture for this device
                        let config = AudioCaptureConfig {
                            sample_rate: Some(audio_source.sample_rate.unwrap_or(48000)),
                            channels: Some(audio_source.channels.unwrap_or(2) as u16),
                        };

                        // Use start_input_capture_for_source which tracks the source mapping
                        match state.audio_capture.start_input_capture_for_source(
                            source_id,
                            &audio_source.device_id,
                            config,
                        ) {
                            Ok(_) => {
                                capture_results.insert(
                                    source_id.clone(),
                                    json!({ "success": true }),
                                );
                            }
                            Err(e) => {
                                capture_results.insert(
                                    source_id.clone(),
                                    json!({ "success": false, "error": e }),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // Start the monitoring loop if not already running
    if !state.audio_level_service.is_running() {
        state
            .audio_level_service
            .start(Arc::new(state.event_bus.clone()));
    }

    Json(json!({
        "ok": true,
        "data": {
            "running": true,
            "trackedSources": req.source_ids.len(),
            "captureResults": capture_results
        }
    }))
}

/// POST /api/audio-levels/stop - Stop audio level monitoring
async fn audio_levels_stop_handler(State(state): State<AppState>) -> impl IntoResponse {
    log::info!("[AudioLevels] Stopping monitoring");
    state.audio_level_service.stop();

    // Clear tracked sources
    state.audio_level_service.set_tracked_sources(vec![]).await;

    Json(json!({
        "ok": true,
        "data": { "running": false }
    }))
}

/// GET /api/audio-levels/health - Get audio level monitoring health status
async fn audio_levels_health_handler(State(state): State<AppState>) -> impl IntoResponse {
    let running = state.audio_level_service.is_running();
    let health = state.audio_level_service.get_health_status().await;

    Json(json!({
        "ok": true,
        "data": {
            "running": running,
            "sources": health
        }
    }))
}

// --- Recording ---

#[derive(Debug, Deserialize)]
struct StartRecordingRequest {
    name: String,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    encrypt: bool,
    #[serde(default)]
    password: Option<String>,
}

/// POST /api/recording/start - Start recording
/// This is a simplified implementation - full version would need to track group info
async fn start_recording_handler(
    State(state): State<AppState>,
    Json(req): Json<StartRecordingRequest>,
) -> impl IntoResponse {
    let format = match req.format.as_deref() {
        Some("mkv") => RecordingFormat::Mkv,
        Some("mov") => RecordingFormat::Mov,
        Some("webm") => RecordingFormat::Webm,
        Some("ts") => RecordingFormat::Ts,
        Some("flv") => RecordingFormat::Flv,
        _ => RecordingFormat::Mp4,
    };

    let config = RecordingConfig {
        name: req.name,
        format,
        encrypt: req.encrypt,
        password: req.password,
    };

    // Get active output group IDs to record from
    let active_ids = state.ffmpeg_handler.get_active_group_ids();
    if active_ids.is_empty() {
        return Json(json!({ "ok": false, "error": "No active streams to record from" }));
    }

    // Use the default passthrough group for recording
    let group = OutputGroup::default();
    let relay_url = format!("rtmp://localhost:1935/relay/{}", active_ids[0]);

    match state.recording_service.start_recording_from_relay(config, &group, &relay_url) {
        Ok(id) => Json(json!({ "ok": true, "data": { "id": id } })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

#[derive(Debug, Deserialize)]
struct StopRecordingRequest {
    id: String,
}

/// POST /api/recording/stop - Stop recording
async fn stop_recording_handler(
    State(state): State<AppState>,
    Json(req): Json<StopRecordingRequest>,
) -> impl IntoResponse {
    match state.recording_service.stop_recording(&req.id) {
        Ok(info) => Json(json!({ "ok": true, "data": info })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

/// GET /api/recordings - List all recordings
async fn list_recordings_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let recordings = state.recording_service.list_recordings();
    Json(json!({ "ok": true, "data": recordings }))
}

#[derive(Debug, Deserialize)]
struct ExportRecordingRequest {
    id: String,
    #[serde(default)]
    password: Option<String>,
    dest_path: String,
}

/// POST /api/recording/export - Export a recording
async fn export_recording_handler(
    State(state): State<AppState>,
    Json(req): Json<ExportRecordingRequest>,
) -> impl IntoResponse {
    match state.recording_service.export_recording(
        &req.id,
        req.password.as_deref(),
        std::path::Path::new(&req.dest_path),
    ) {
        Ok(()) => Json(json!({ "ok": true, "data": null })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

/// DELETE /api/recording/:id - Delete a recording
async fn delete_recording_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.recording_service.delete_recording(&id) {
        Ok(()) => Json(json!({ "ok": true, "data": null })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

// --- Replay Buffer ---

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartReplayBufferRequest {
    duration_secs: Option<u32>,
    output_path: Option<String>,
}

/// POST /api/replay-buffer/start - Start the replay buffer
async fn start_replay_buffer_handler(
    State(state): State<AppState>,
    Json(req): Json<StartReplayBufferRequest>,
) -> impl IntoResponse {
    // Get active output group IDs to record from
    let active_ids = state.ffmpeg_handler.get_active_group_ids();
    if active_ids.is_empty() {
        return Json(json!({ "ok": false, "error": "No active streams - start streaming first" }));
    }

    let relay_url = format!("rtmp://localhost:1935/relay/{}", active_ids[0]);

    let config = ReplayBufferConfig {
        duration_secs: req.duration_secs.unwrap_or(30),
        output_path: req.output_path.unwrap_or_default(),
        segment_duration: 2,
    };

    match state.replay_buffer.start(&relay_url, config) {
        Ok(()) => Json(json!({ "ok": true, "data": null })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

/// POST /api/replay-buffer/stop - Stop the replay buffer
async fn stop_replay_buffer_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.replay_buffer.stop() {
        Ok(()) => Json(json!({ "ok": true, "data": null })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

/// POST /api/replay-buffer/save - Save the current replay buffer
async fn save_replay_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.replay_buffer.save_replay() {
        Ok(info) => Json(json!({ "ok": true, "data": info })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

/// GET /api/replay-buffer/state - Get replay buffer state
async fn get_replay_buffer_state_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.replay_buffer.get_state() {
        Ok(buffer_state) => Json(json!({ "ok": true, "data": buffer_state })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetReplayDurationRequest {
    duration_secs: u32,
}

/// POST /api/replay-buffer/duration - Set replay buffer duration
async fn set_replay_duration_handler(
    State(state): State<AppState>,
    Json(req): Json<SetReplayDurationRequest>,
) -> impl IntoResponse {
    match state.replay_buffer.set_duration(req.duration_secs) {
        Ok(()) => Json(json!({ "ok": true, "data": null })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

#[derive(Debug, Deserialize)]
struct SetReplayOutputPathRequest {
    path: String,
}

/// POST /api/replay-buffer/output-path - Set replay buffer output path
async fn set_replay_output_path_handler(
    State(state): State<AppState>,
    Json(req): Json<SetReplayOutputPathRequest>,
) -> impl IntoResponse {
    match state.replay_buffer.set_output_path(req.path) {
        Ok(()) => Json(json!({ "ok": true, "data": null })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

// --- Permissions ---

/// GET /api/permissions/status - Get permission status
/// Uses async-safe permission checks that run scap calls in spawn_blocking on macOS
async fn permissions_status_handler() -> impl IntoResponse {
    let status = PermissionsService::get_status_async().await;
    Json(json!({ "ok": true, "data": status }))
}

#[derive(Debug, Deserialize)]
struct RequestPermissionsRequest {
    types: Vec<String>,
}

/// POST /api/permissions/request - Request permissions
/// On macOS, screen recording permission opens System Preferences
/// On Windows/Linux, permissions are handled via picker dialogs when capture starts
async fn request_permissions_handler(
    Json(req): Json<RequestPermissionsRequest>,
) -> impl IntoResponse {
    let mut results = std::collections::HashMap::new();

    for perm_type in &req.types {
        let granted = match perm_type.as_str() {
            "camera" => PermissionsService::request_camera_permission(),
            "microphone" => PermissionsService::request_microphone_permission(),
            "screen" | "screenRecording" => {
                PermissionsService::request_screen_recording_permission_async().await
            }
            _ => false,
        };
        results.insert(perm_type.clone(), granted);
    }

    let status = PermissionsService::get_status_async().await;
    Json(json!({ "ok": true, "data": { "results": results, "status": status } }))
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

/// Graceful shutdown signal handler
/// Waits for Ctrl+C or SIGTERM, then stops all services in order
async fn shutdown_signal(state: AppState) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    log::info!("Shutdown signal received, stopping services...");

    // Stop services in dependency order (most critical first)

    // 1. Stop active streams (FFmpeg processes)
    if let Err(e) = state.ffmpeg_handler.stop_all() {
        log::warn!("Error stopping FFmpeg streams: {}", e);
    }

    // 2. Stop recording and replay buffer
    let _ = state.recording_service.stop_all();
    if let Err(e) = state.replay_buffer.stop() {
        log::warn!("Error stopping replay buffer: {}", e);
    }

    // 3. Stop WebRTC/go2rtc
    state.go2rtc_manager.stop().await;

    // 4. Stop all preview handlers
    state.preview_handler.stop_all_previews();
    state.native_preview.stop_all();

    // 5. Stop capture services
    state.screen_capture.stop_all();
    state.camera_capture.stop_all();
    state.audio_capture.stop_all();
    state.h264_capture.stop_all();

    // 6. Stop audio level monitoring
    state.audio_level_service.stop();
    state.audio_level_extractor.stop_all();

    log::info!("All services stopped, server shutting down");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
        custom_ffmpeg_path.clone(),
    ));

    // Create preview handler using the same FFmpeg path
    let preview_ffmpeg_path = custom_ffmpeg_path
        .clone()
        .filter(|p| !p.is_empty() && std::path::Path::new(p).exists())
        .unwrap_or_else(|| ffmpeg_handler.get_ffmpeg_path());
    let preview_handler = Arc::new(PreviewHandler::new(preview_ffmpeg_path.clone()));

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

    // NOTE: We do NOT call PermissionsService::init_permission_cache() here.
    // The scap permission check can block/hang for 3-10 seconds and calling it
    // within the tokio runtime starves other tasks. Permission checks are done
    // lazily via spawn_blocking when actually needed.

    // Initialize native capture services
    let screen_capture = Arc::new(ScreenCaptureService::new());
    let audio_capture = Arc::new(AudioCaptureService::new());
    // Pre-warm audio device cache in background for faster first capture
    {
        let audio_capture_warmup = audio_capture.clone();
        tokio::task::spawn_blocking(move || {
            audio_capture_warmup.warm_cache();
        });
    }
    let camera_capture = Arc::new(CameraCaptureService::new(preview_ffmpeg_path.clone()));
    let native_preview = Arc::new(NativePreviewService::new());
    let recording_service = Arc::new(
        RecordingService::new(preview_ffmpeg_path.clone(), app_data_dir.clone())
            .expect("Failed to initialize recording service")
    );
    let replay_buffer = Arc::new(
        ReplayBufferService::new(preview_ffmpeg_path.clone(), app_data_dir.clone())
            .expect("Failed to initialize replay buffer service")
    );
    let capture_indicator = Arc::new(CaptureIndicatorService::new());

    // Initialize go2rtc manager for WebRTC preview streaming
    let go2rtc_manager = Arc::new(Go2RtcManager::new());
    // Try to start go2rtc in background (non-blocking, will set is_available when ready)
    {
        let manager = go2rtc_manager.clone();
        tokio::spawn(async move {
            match manager.start().await {
                Ok(()) => log::info!("go2rtc WebRTC server started successfully"),
                Err(e) => log::warn!("go2rtc not available (WebRTC preview disabled): {}", e),
            }
        });
    }

    // Initialize H264 capture service for native screen capture → WebRTC
    let h264_capture = Arc::new(H264CaptureService::new(
        screen_capture.clone(),
        preview_ffmpeg_path.clone(),
    ));

    // Initialize audio level monitoring service
    let audio_level_service = Arc::new(AudioLevelService::new());
    // Initialize audio level extractor for FFmpeg-based sources
    let audio_level_extractor = Arc::new(AudioLevelExtractor::new(preview_ffmpeg_path.clone()));
    // Initialize ScreenCaptureKit audio capture service (macOS only)
    #[cfg(target_os = "macos")]
    let sck_audio_capture = Arc::new(SckAudioCaptureService::new());
    // Clone event_bus for audio level service before it's moved into state
    let event_bus_for_audio = event_bus.clone();

    let state = AppState {
        profile_manager,
        settings_manager,
        ffmpeg_handler,
        ffmpeg_downloader: Arc::new(AsyncMutex::new(FFmpegDownloader::new())),
        theme_manager,
        preview_handler,
        event_bus,
        log_dir: log_dir_path,
        app_data_dir,
        auth_token,
        rate_limiter,
        home_dir,
        // Native capture services
        screen_capture,
        audio_capture,
        camera_capture,
        native_preview,
        recording_service,
        replay_buffer,
        capture_indicator,
        go2rtc_manager,
        h264_capture,
        audio_level_service: audio_level_service.clone(),
        audio_level_extractor,
        #[cfg(target_os = "macos")]
        sck_audio_capture,
        server_port: port,
    };

    // Start audio level monitoring service
    audio_level_service.start(Arc::new(event_bus_for_audio));

    // Build CORS layer
    let cors = build_cors_layer();

    // Build CSP header (allow blob: for preview images, Google Fonts, and inline scripts)
    let csp_value = HeaderValue::from_static(
        "default-src 'self'; \
         script-src 'self' 'unsafe-inline'; \
         style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; \
         font-src 'self' https://fonts.gstatic.com; \
         connect-src 'self' ws://localhost:* wss://localhost:* http://localhost:* http://127.0.0.1:* ws://127.0.0.1:*; \
         img-src 'self' data: blob: http://localhost:* http://127.0.0.1:*; \
         media-src 'self' blob:; \
         frame-src 'self' http://127.0.0.1:* http://localhost:* https://* http://*"
    );

    // Build router with security layers
    // Protected routes (require authentication)
    let protected_routes = Router::new()
        .route("/api/invoke/:command", post(invoke))
        .route("/ws", get(ws_handler))
        .route("/ws/preview/:source_id", get(ws_preview_handler))
        // File browser endpoints for HTTP mode dialogs
        .route("/api/files/browse", get(files_browse))
        .route("/api/files/home", get(files_home))
        .route("/api/files/open", post(files_open))
        .route("/api/system/default-paths", get(system_default_paths))
        // Static file serving (images, HTML)
        .route("/api/static", get(static_file_handler))
        // Preview endpoints (MJPEG/snapshot)
        .route("/api/preview/source/:source_id", get(source_preview_handler))
        .route("/api/preview/source/:source_id/snapshot", get(source_snapshot_handler))
        .route("/api/preview/source/:source_id/stop", post(stop_source_preview_handler))
        .route("/api/preview/stop-all", post(stop_all_previews_handler))
        // Scene preview endpoints (composed output)
        .route("/api/preview/scene/:profile/:scene_id", get(scene_preview_handler))
        .route("/api/preview/scene/:profile/:scene_id/snapshot", get(scene_snapshot_handler))
        .route("/api/preview/scene/stop", post(stop_scene_preview_handler))
        // Device discovery endpoints
        .route("/api/devices/cameras", get(list_cameras_handler))
        .route("/api/devices/displays", get(list_displays_handler))
        .route("/api/devices/audio/input", get(list_audio_input_handler))
        .route("/api/devices/audio/output", get(list_audio_output_handler))
        .route("/api/devices/capture-cards", get(list_capture_cards_handler))
        .route("/api/devices/windows", get(list_windows_handler))
        // Capture control endpoints
        .route("/api/capture/camera/start", post(start_camera_capture_handler))
        .route("/api/capture/camera/stop", post(stop_camera_capture_handler))
        .route("/api/capture/screen/start", post(start_screen_capture_handler))
        .route("/api/capture/screen/stop", post(stop_screen_capture_handler))
        .route("/api/capture/audio/start", post(start_audio_capture_handler))
        .route("/api/capture/audio/stop", post(stop_audio_capture_handler))
        .route("/api/capture/status", get(capture_status_handler))
        // H264 MPEG-TS streaming endpoint (for go2rtc #video=copy passthrough)
        .route("/api/capture/:source_id/stream", get(capture_stream_handler))
        // Audio level monitoring endpoints
        .route("/api/audio-levels/start", post(audio_levels_start_handler))
        .route("/api/audio-levels/stop", post(audio_levels_stop_handler))
        .route("/api/audio-levels/health", get(audio_levels_health_handler))
        // Recording endpoints
        .route("/api/recording/start", post(start_recording_handler))
        .route("/api/recording/stop", post(stop_recording_handler))
        .route("/api/recordings", get(list_recordings_handler))
        .route("/api/recording/export", post(export_recording_handler))
        .route("/api/recording/:id", axum::routing::delete(delete_recording_handler))
        // Replay Buffer endpoints
        .route("/api/replay-buffer/start", post(start_replay_buffer_handler))
        .route("/api/replay-buffer/stop", post(stop_replay_buffer_handler))
        .route("/api/replay-buffer/save", post(save_replay_handler))
        .route("/api/replay-buffer/state", get(get_replay_buffer_state_handler))
        .route("/api/replay-buffer/duration", post(set_replay_duration_handler))
        .route("/api/replay-buffer/output-path", post(set_replay_output_path_handler))
        // Permissions endpoints
        .route("/api/permissions/status", get(permissions_status_handler))
        .route("/api/permissions/request", post(request_permissions_handler))
        // WebRTC preview endpoints (go2rtc integration)
        .route("/api/webrtc/available", get(webrtc_available_handler))
        .route("/api/webrtc/info/:source_id", get(webrtc_info_handler))
        .route("/api/webrtc/start/:source_id", post(webrtc_start_handler))
        .route("/api/webrtc/stop/:source_id", post(webrtc_stop_handler))
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

    // Run server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(state))
        .await?;

    Ok(())
}
