use axum::{
    extract::{Json, State},
    response::IntoResponse,
};
use serde_json::json;

use crate::app_state::AppState;
use crate::services::{DeviceDiscovery, ScreenCaptureService};

/// GET /api/devices/cameras - List available cameras
pub(crate) async fn list_cameras_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let cameras = state.camera_capture.list_cameras();
    Json(json!({ "ok": true, "data": cameras }))
}

/// GET /api/devices/displays - List available displays
/// Uses async version with spawn_blocking and timeout protection
pub(crate) async fn list_displays_handler() -> impl IntoResponse {
    let displays = ScreenCaptureService::list_displays_async().await;
    Json(json!({ "ok": true, "data": displays }))
}

/// GET /api/devices/audio/input - List audio input devices (microphones)
pub(crate) async fn list_audio_input_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let devices = state.audio_capture.list_input_devices();
    Json(json!({ "ok": true, "data": devices }))
}

/// GET /api/devices/audio/output - List audio output devices
pub(crate) async fn list_audio_output_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let devices = state.audio_capture.list_output_devices();
    Json(json!({ "ok": true, "data": devices }))
}

/// GET /api/devices/capture-cards - List available capture cards (Elgato, etc.)
pub(crate) async fn list_capture_cards_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let discovery = DeviceDiscovery::with_cache(state.ffmpeg_handler.get_ffmpeg_path(), state.device_cache.clone());
    match discovery.list_capture_cards_async().await {
        Ok(cards) => Json(json!({ "ok": true, "data": cards })),
        Err(e) => Json(json!({ "ok": false, "error": format!("Failed to list capture cards: {}", e) })),
    }
}

/// GET /api/devices/windows - List capturable windows
/// Uses ScreenCaptureKit on macOS for window enumeration
pub(crate) async fn list_windows_handler() -> impl IntoResponse {
    let windows = ScreenCaptureService::list_windows_async().await;
    Json(json!({ "ok": true, "data": windows }))
}
