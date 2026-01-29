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
use tower_cookies::{Cookie, CookieManagerLayer, Cookies};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
};

use spiritstream_server::commands::{get_encoders, test_ffmpeg, test_rtmp_target, validate_ffmpeg_path};
use spiritstream_server::models::{
    OutputGroup, Profile, RtmpInput, Settings, Source, Scene, SourceLayer, Transform, AudioTrack,
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
    CaptureIndicatorService, CaptureType,
    PermissionsService,
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
    capture_indicator: Arc<CaptureIndicatorService>,
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
        // Device Discovery Commands
        // ====================================================================

        "list_cameras" => {
            let discovery = DeviceDiscovery::new(state.ffmpeg_handler.get_ffmpeg_path());
            let cameras = discovery.list_cameras()?;
            Ok(json!(cameras))
        }
        "list_displays" => {
            let discovery = DeviceDiscovery::new(state.ffmpeg_handler.get_ffmpeg_path());
            let displays = discovery.list_displays()?;
            Ok(json!(displays))
        }
        "list_audio_devices" => {
            let discovery = DeviceDiscovery::new(state.ffmpeg_handler.get_ffmpeg_path());
            let devices = discovery.list_audio_inputs()?;
            Ok(json!(devices))
        }
        "list_capture_cards" => {
            let discovery = DeviceDiscovery::new(state.ffmpeg_handler.get_ffmpeg_path());
            let cards = discovery.list_capture_cards()?;
            Ok(json!(cards))
        }
        "refresh_devices" => {
            // Return all device types at once
            let discovery = DeviceDiscovery::new(state.ffmpeg_handler.get_ffmpeg_path());
            let cameras = discovery.list_cameras().unwrap_or_default();
            let displays = discovery.list_displays().unwrap_or_default();
            let audio = discovery.list_audio_inputs().unwrap_or_default();
            let capture_cards = discovery.list_capture_cards().unwrap_or_default();
            Ok(json!({
                "cameras": cameras,
                "displays": displays,
                "audioDevices": audio,
                "captureCards": capture_cards
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

            let mut profile = state
                .profile_manager
                .load_with_key_decryption(&profile_name, password.as_deref())
                .await?;

            let initial_len = profile.sources.len();
            profile.sources.retain(|s| s.id() != source_id);

            if profile.sources.len() == initial_len {
                return Err(format!("Source {} not found", source_id));
            }

            // Stop any running preview for this source
            state.preview_handler.stop_source_preview(&source_id);

            // Also remove from all scenes
            for scene in &mut profile.scenes {
                scene.layers.retain(|l| l.source_id != source_id);
                scene.audio_mixer.tracks.retain(|t| t.source_id != source_id);
            }

            let settings = state.settings_manager.load()?;
            state
                .profile_manager
                .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
                .await?;

            state.event_bus.emit("source_removed", json!({ "profileName": profile_name, "sourceId": source_id }));
            Ok(Value::Null)
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

            let scene = profile.scenes.iter_mut().find(|s| s.id == scene_id)
                .ok_or_else(|| format!("Scene {} not found", scene_id))?;

            let max_z = scene.layers.iter().map(|l| l.z_index).max().unwrap_or(0);
            let layer = SourceLayer {
                id: uuid::Uuid::new_v4().to_string(),
                source_id: source_id.clone(),
                visible: true,
                locked: false,
                transform: transform.unwrap_or_else(|| Transform {
                    x: 0,
                    y: 0,
                    width: scene.canvas_width,
                    height: scene.canvas_height,
                    rotation: 0.0,
                    crop: None,
                }),
                z_index: max_z + 1,
            };

            let layer_id = layer.id.clone();
            scene.layers.push(layer);

            // Add audio track if source has audio and not already in mixer
            if !scene.audio_mixer.tracks.iter().any(|t| t.source_id == source_id) {
                scene.audio_mixer.tracks.push(AudioTrack {
                    source_id,
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

            state.event_bus.emit("layer_added", json!({ "profileName": profile_name, "sceneId": scene_id, "layerId": layer_id }));
            Ok(json!({ "layerId": layer_id }))
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
    let camera_capture = Arc::new(CameraCaptureService::new(preview_ffmpeg_path.clone()));
    let native_preview = Arc::new(NativePreviewService::new());
    let recording_service = Arc::new(
        RecordingService::new(preview_ffmpeg_path.clone(), app_data_dir.clone())
            .expect("Failed to initialize recording service")
    );
    let capture_indicator = Arc::new(CaptureIndicatorService::new());

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
        capture_indicator,
    };

    // Build CORS layer
    let cors = build_cors_layer();

    // Build CSP header (allow blob: for preview images)
    let csp_value = HeaderValue::from_static(
        "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; \
         connect-src 'self' ws://localhost:* wss://localhost:* http://localhost:* http://127.0.0.1:*; \
         img-src 'self' data: blob: http://localhost:* http://127.0.0.1:*; font-src 'self'"
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
        // Capture control endpoints
        .route("/api/capture/camera/start", post(start_camera_capture_handler))
        .route("/api/capture/camera/stop", post(stop_camera_capture_handler))
        .route("/api/capture/screen/start", post(start_screen_capture_handler))
        .route("/api/capture/screen/stop", post(stop_screen_capture_handler))
        .route("/api/capture/audio/start", post(start_audio_capture_handler))
        .route("/api/capture/audio/stop", post(stop_audio_capture_handler))
        .route("/api/capture/status", get(capture_status_handler))
        // Recording endpoints
        .route("/api/recording/start", post(start_recording_handler))
        .route("/api/recording/stop", post(stop_recording_handler))
        .route("/api/recordings", get(list_recordings_handler))
        .route("/api/recording/export", post(export_recording_handler))
        .route("/api/recording/:id", axum::routing::delete(delete_recording_handler))
        // Permissions endpoints
        .route("/api/permissions/status", get(permissions_status_handler))
        .route("/api/permissions/request", post(request_permissions_handler))
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
