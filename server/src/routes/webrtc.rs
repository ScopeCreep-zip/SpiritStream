use axum::{
    extract::{Json, Path, State},
    http::HeaderMap,
    response::IntoResponse,
};
use serde_json::json;
use std::time::Duration;
use tower_cookies::Cookies;

use crate::app_state::AppState;
use crate::middleware::{bearer_token, is_valid_session, verify_token, AUTH_COOKIE_NAME};
use crate::models::Source;
use crate::services::{unavailable_webrtc_info, EventSink};
#[cfg(target_os = "macos")]
use crate::services::ScreenCaptureService;

/// GET /api/webrtc/available - Check if go2rtc WebRTC server is available
pub(crate) async fn webrtc_available_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let available = state.go2rtc_manager.is_available();
    Json(json!({ "ok": true, "data": available }))
}

/// GET /api/webrtc/info/:source_id - Get WebRTC streaming info for a source
pub(crate) async fn webrtc_info_handler(
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
pub(crate) async fn webrtc_start_handler(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
    cookies: Cookies,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Authentication check
    if let Some(expected) = state.auth_token.as_deref() {
        let authenticated = cookies.get(AUTH_COOKIE_NAME).is_some_and(|c| is_valid_session(c.value()))
            || bearer_token(&headers).is_some_and(|t| verify_token(expected, t));

        if !authenticated {
            return Json(json!({ "ok": false, "error": "Unauthorized" }));
        }
    }

    // Lazy-start go2rtc on first WebRTC request
    if let Err(e) = state.go2rtc_manager.ensure_started().await {
        return Json(json!({ "ok": false, "error": format!("go2rtc not available: {}", e) }));
    }
    // Notify frontend that go2rtc is now available
    state.event_bus.emit("go2rtc_status", json!({ "available": true }));

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

    log::info!(
        "WebRTC start for source '{}' — HW encoders: {}/{}, active H264: {}, active screen: {}",
        source_id,
        state.h264_capture.hw_budget().active_count(),
        state.h264_capture.hw_budget().max_sessions(),
        state.h264_capture.active_captures().len(),
        state.screen_capture.active_capture_count()
    );

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
                    log::error!("Failed to start H264 capture for {}: {}", source_id, e);
                    return Json(json!({ "ok": false, "error": format!("Failed to start screen capture: {}", e) }));
                }
                Ok(Err(_panic)) => {
                    log::error!("H264 capture panicked for source {}", source_id);
                    return Json(json!({ "ok": false, "error": "Screen capture failed unexpectedly (panic)" }));
                }
                Err(join_err) => {
                    log::error!("H264 capture task failed: {}", join_err);
                    return Json(json!({ "ok": false, "error": "Screen capture task failed" }));
                }
            }

            // Wait for first MPEG-TS data before registering with go2rtc.
            // This ensures go2rtc's Dial() will get valid MPEG-TS headers when it
            // connects to our HTTP stream endpoint — prevents WHEP 500 race condition.
            match state.h264_capture.subscribe_to_stream(&source_id) {
                Some(mut rx) => {
                    match tokio::time::timeout(Duration::from_secs(10), rx.recv()).await {
                        Ok(Ok(_)) => log::debug!("First MPEG-TS data confirmed for {}", source_id),
                        Ok(Err(e)) => log::warn!("Stream recv error for {}: {}", source_id, e),
                        Err(_) => log::warn!("Timeout waiting for first MPEG-TS data from {} (10s) — stream may be slow to start", source_id),
                    }
                }
                None => {
                    log::error!("No H264 session for {} — capture did not start correctly", source_id);
                    return Json(json!({ "ok": false, "error": "Screen capture session not found after start" }));
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
                let _ = win;
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
pub(crate) async fn webrtc_stop_handler(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
    cookies: Cookies,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Authentication check
    if let Some(expected) = state.auth_token.as_deref() {
        let authenticated = cookies.get(AUTH_COOKIE_NAME).is_some_and(|c| is_valid_session(c.value()))
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
