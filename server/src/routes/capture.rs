use axum::{
    extract::{Json, Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;

use crate::app_state::AppState;
use crate::services::{
    AudioCaptureConfig, CameraCaptureConfig, CaptureType, ScreenCaptureConfig,
};

#[derive(Debug, Deserialize)]
pub(crate) struct CameraCaptureRequest {
    device_id: String,
}

/// POST /api/capture/camera/start - Start camera capture
pub(crate) async fn start_camera_capture_handler(
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
pub(crate) async fn stop_camera_capture_handler(
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
pub(crate) struct ScreenCaptureRequest {
    display_id: String,
}

/// POST /api/capture/screen/start - Start screen capture
/// Uses spawn_blocking because scap::get_all_targets() can block/hang on macOS
pub(crate) async fn start_screen_capture_handler(
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
pub(crate) async fn stop_screen_capture_handler(
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
pub(crate) struct AudioCaptureRequest {
    device_id: String,
    #[serde(default)]
    is_loopback: bool,
}

/// POST /api/capture/audio/start - Start audio capture
pub(crate) async fn start_audio_capture_handler(
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
pub(crate) async fn stop_audio_capture_handler(
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
pub(crate) async fn capture_status_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let status = state.capture_indicator.get_status();
    Json(json!({ "ok": true, "data": status }))
}

/// GET /api/capture/{source_id}/stream - Stream MPEG-TS H264 capture to go2rtc
/// This endpoint serves the H264 encoded MPEG-TS stream from our capture service.
/// go2rtc uses this with #video=copy to passthrough without re-encoding.
pub(crate) async fn capture_stream_handler(
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
    // Includes 10s inactivity timeout to prevent orphaned receivers from leaking memory
    let stream = async_stream::stream! {
        let mut rx = rx;
        loop {
            match tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv()).await {
                Ok(Ok(chunk)) => {
                    yield Ok::<_, std::io::Error>(chunk);
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(n))) => {
                    log::warn!("Stream consumer lagged by {} chunks for {}", n, source_id);
                    // Continue receiving
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    log::info!("H264 capture stream closed for {}", source_id);
                    break;
                }
                Err(_timeout) => {
                    log::info!("H264 stream receiver timed out (10s no data) for {}, closing", source_id);
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
