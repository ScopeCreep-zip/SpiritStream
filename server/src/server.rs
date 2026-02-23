use axum::{
    http::{header, HeaderValue},
    middleware,
    routing::{get, post},
    Router,
};
use std::net::{IpAddr, Ipv4Addr};
use tower_cookies::CookieManagerLayer;
use tower_http::{
    set_header::SetResponseHeaderLayer,
};

use crate::app_state::AppState;
use crate::middleware as app_middleware;
use crate::routes;

pub fn parse_host(host: &str) -> IpAddr {
    host.parse().unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}

/// Find themes directory by searching common relative paths from CWD.
/// Used as fallback when SPIRITSTREAM_THEMES_DIR is not set or invalid.
pub fn find_themes_dir_fallback() -> Option<String> {
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

pub(crate) fn build_csp_header() -> HeaderValue {
    HeaderValue::from_static(
        "default-src 'self'; \
         script-src 'self' 'unsafe-inline'; \
         style-src 'self' 'unsafe-inline'; \
         font-src 'self'; \
         connect-src 'self' ws://localhost:* wss://localhost:* http://localhost:* http://127.0.0.1:* ws://127.0.0.1:*; \
         img-src 'self' data: blob: http://localhost:* http://127.0.0.1:*; \
         media-src 'self' blob:; \
         frame-src 'self' http://127.0.0.1:* http://localhost:* https://* http://*"
    )
}

/// Build the full Axum router with all routes and middleware layers.
pub fn build_router(state: AppState) -> Router {
    let cors = app_middleware::build_cors_layer();
    let csp_value = build_csp_header();

    // Protected routes (require authentication)
    let protected_routes = Router::new()
        .route("/api/invoke/:command", post(routes::invoke))
        .route("/ws", get(routes::ws_handler))
        .route("/ws/preview/:source_id", get(routes::ws_preview_handler))
        // File browser endpoints for HTTP mode dialogs
        .route("/api/files/browse", get(routes::files_browse))
        .route("/api/files/home", get(routes::files_home))
        .route("/api/files/open", post(routes::files_open))
        .route("/api/system/default-paths", get(routes::system_default_paths))
        // Static file serving (images, HTML)
        .route("/api/static", get(routes::static_file_handler))
        // Preview endpoints (MJPEG/snapshot)
        .route("/api/preview/source/:source_id", get(routes::source_preview_handler))
        .route("/api/preview/source/:source_id/snapshot", get(routes::source_snapshot_handler))
        .route("/api/preview/source/:source_id/stop", post(routes::stop_source_preview_handler))
        .route("/api/preview/stop-all", post(routes::stop_all_previews_handler))
        // Scene preview endpoints (composed output)
        .route("/api/preview/scene/:profile/:scene_id", get(routes::scene_preview_handler))
        .route("/api/preview/scene/:profile/:scene_id/snapshot", get(routes::scene_snapshot_handler))
        .route("/api/preview/scene/stop", post(routes::stop_scene_preview_handler))
        // Device discovery endpoints
        .route("/api/devices/cameras", get(routes::list_cameras_handler))
        .route("/api/devices/displays", get(routes::list_displays_handler))
        .route("/api/devices/audio/input", get(routes::list_audio_input_handler))
        .route("/api/devices/audio/output", get(routes::list_audio_output_handler))
        .route("/api/devices/capture-cards", get(routes::list_capture_cards_handler))
        .route("/api/devices/windows", get(routes::list_windows_handler))
        // Capture control endpoints
        .route("/api/capture/camera/start", post(routes::start_camera_capture_handler))
        .route("/api/capture/camera/stop", post(routes::stop_camera_capture_handler))
        .route("/api/capture/screen/start", post(routes::start_screen_capture_handler))
        .route("/api/capture/screen/stop", post(routes::stop_screen_capture_handler))
        .route("/api/capture/audio/start", post(routes::start_audio_capture_handler))
        .route("/api/capture/audio/stop", post(routes::stop_audio_capture_handler))
        .route("/api/capture/status", get(routes::capture_status_handler))
        // H264 MPEG-TS streaming endpoint (for go2rtc #video=copy passthrough)
        .route("/api/capture/:source_id/stream", get(routes::capture_stream_handler))
        // Audio level monitoring endpoints
        .route("/api/audio-levels/start", post(routes::audio_levels_start_handler))
        .route("/api/audio-levels/stop", post(routes::audio_levels_stop_handler))
        .route("/api/audio-levels/health", get(routes::audio_levels_health_handler))
        // Audio filter endpoints
        .route("/api/audio/filters/:source_id", post(routes::set_audio_filters_handler))
        .route("/api/audio/filters/:source_id", get(routes::get_audio_filters_handler))
        // Recording endpoints
        .route("/api/recording/start", post(routes::start_recording_handler))
        .route("/api/recording/stop", post(routes::stop_recording_handler))
        .route("/api/recordings", get(routes::list_recordings_handler))
        .route("/api/recording/export", post(routes::export_recording_handler))
        .route("/api/recording/:id", axum::routing::delete(routes::delete_recording_handler))
        // Replay Buffer endpoints
        .route("/api/replay-buffer/start", post(routes::start_replay_buffer_handler))
        .route("/api/replay-buffer/stop", post(routes::stop_replay_buffer_handler))
        .route("/api/replay-buffer/save", post(routes::save_replay_handler))
        .route("/api/replay-buffer/state", get(routes::get_replay_buffer_state_handler))
        .route("/api/replay-buffer/duration", post(routes::set_replay_duration_handler))
        .route("/api/replay-buffer/output-path", post(routes::set_replay_output_path_handler))
        // Permissions endpoints
        .route("/api/permissions/status", get(routes::permissions_status_handler))
        .route("/api/permissions/request", post(routes::request_permissions_handler))
        // System status endpoints
        .route("/api/system/power", get(routes::power_status))
        // WebRTC preview endpoints (go2rtc integration)
        .route("/api/webrtc/available", get(routes::webrtc_available_handler))
        .route("/api/webrtc/info/:source_id", get(routes::webrtc_info_handler))
        .route("/api/webrtc/start/:source_id", post(routes::webrtc_start_handler))
        .route("/api/webrtc/stop/:source_id", post(routes::webrtc_stop_handler))
        .layer(middleware::from_fn_with_state(state.clone(), app_middleware::auth_middleware));

    // Public routes (no auth required)
    let public_routes = Router::new()
        .route("/health", get(routes::health))
        .route("/ready", get(routes::ready))
        .route("/auth/login", post(app_middleware::auth_login))
        .route("/auth/logout", post(app_middleware::auth_logout))
        .route("/auth/check", get(app_middleware::auth_check));

    // Combine all routes
    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state, app_middleware::rate_limit_middleware))
        .layer(CookieManagerLayer::new())
        .layer(cors)
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CONTENT_SECURITY_POLICY,
            csp_value,
        ))
}
