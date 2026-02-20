use axum::{
    extract::{Json, Path, State},
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;

use crate::app_state::AppState;
use crate::models::OutputGroup;
use crate::services::{RecordingConfig, RecordingFormat, ReplayBufferConfig, validate_path_within};

#[derive(Debug, Deserialize)]
pub(crate) struct StartRecordingRequest {
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
pub(crate) async fn start_recording_handler(
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
pub(crate) struct StopRecordingRequest {
    id: String,
}

/// POST /api/recording/stop - Stop recording
pub(crate) async fn stop_recording_handler(
    State(state): State<AppState>,
    Json(req): Json<StopRecordingRequest>,
) -> impl IntoResponse {
    match state.recording_service.stop_recording(&req.id) {
        Ok(info) => Json(json!({ "ok": true, "data": info })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

/// GET /api/recordings - List all recordings
pub(crate) async fn list_recordings_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let recordings = state.recording_service.list_recordings();
    Json(json!({ "ok": true, "data": recordings }))
}

#[derive(Debug, Deserialize)]
pub(crate) struct ExportRecordingRequest {
    id: String,
    #[serde(default)]
    password: Option<String>,
    dest_path: String,
}

/// POST /api/recording/export - Export a recording
pub(crate) async fn export_recording_handler(
    State(state): State<AppState>,
    Json(req): Json<ExportRecordingRequest>,
) -> impl IntoResponse {
    // Validate destination path against home directory to prevent path traversal
    let dest_path = std::path::Path::new(&req.dest_path);
    if let Some(home) = state.home_dir.as_deref() {
        if let Err(e) = validate_path_within(dest_path, home) {
            return Json(json!({ "ok": false, "error": e }));
        }
    }

    match state.recording_service.export_recording(
        &req.id,
        req.password.as_deref(),
        dest_path,
    ) {
        Ok(()) => Json(json!({ "ok": true, "data": null })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

/// DELETE /api/recording/:id - Delete a recording
pub(crate) async fn delete_recording_handler(
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
pub(crate) struct StartReplayBufferRequest {
    duration_secs: Option<u32>,
    output_path: Option<String>,
}

/// POST /api/replay-buffer/start - Start the replay buffer
pub(crate) async fn start_replay_buffer_handler(
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
pub(crate) async fn stop_replay_buffer_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.replay_buffer.stop() {
        Ok(()) => Json(json!({ "ok": true, "data": null })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

/// POST /api/replay-buffer/save - Save the current replay buffer
pub(crate) async fn save_replay_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.replay_buffer.save_replay() {
        Ok(info) => Json(json!({ "ok": true, "data": info })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

/// GET /api/replay-buffer/state - Get replay buffer state
pub(crate) async fn get_replay_buffer_state_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.replay_buffer.get_state() {
        Ok(buffer_state) => Json(json!({ "ok": true, "data": buffer_state })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetReplayDurationRequest {
    duration_secs: u32,
}

/// POST /api/replay-buffer/duration - Set replay buffer duration
pub(crate) async fn set_replay_duration_handler(
    State(state): State<AppState>,
    Json(req): Json<SetReplayDurationRequest>,
) -> impl IntoResponse {
    match state.replay_buffer.set_duration(req.duration_secs) {
        Ok(()) => Json(json!({ "ok": true, "data": null })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct SetReplayOutputPathRequest {
    path: String,
}

/// POST /api/replay-buffer/output-path - Set replay buffer output path
pub(crate) async fn set_replay_output_path_handler(
    State(state): State<AppState>,
    Json(req): Json<SetReplayOutputPathRequest>,
) -> impl IntoResponse {
    // Validate output path against home directory to prevent path traversal
    let output_path = std::path::Path::new(&req.path);
    if let Some(home) = state.home_dir.as_deref() {
        if let Err(e) = validate_path_within(output_path, home) {
            return Json(json!({ "ok": false, "error": e }));
        }
    }

    match state.replay_buffer.set_output_path(req.path) {
        Ok(()) => Json(json!({ "ok": true, "data": null })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}
