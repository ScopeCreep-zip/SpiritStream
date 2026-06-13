//! Stream handlers — `/api/v1/streams/*`.
//!
//! Validate + status + per-group lifecycle (start / start_all / stop /
//! stop_all / retry / toggle_target) + the per-target disabled flag
//! query. Routes hit `FFmpegHandler` / `StreamService`; the OBS and
//! chat cascades are wired into `start_all` / `stop_all` by
//! `chat_lifecycle` and the OBS trigger registered at startup.

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::AppState;

// Streams.
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StreamValidateRequest {
    /// Full profile body. Runtime type is the core `Profile`; OpenAPI
    /// schema is `ProfileWire`. Encoding-config rules (bitrate /
    /// keyframe / resolution / fps) are evaluated server-side.
    #[schema(value_type = ProfileWire)]
    pub profile: spiritstream_core::models::Profile,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StreamValidateResponse {
    /// `true` when every output group passes bound checks. `false` when one
    /// or more `ValidationIssue`s would be returned by `POST /streams`.
    pub valid: bool,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StreamStatusResponse {
    /// Group IDs of every output group with at least one active FFmpeg process.
    pub active_group_ids: Vec<String>,
    /// Convenience count — same as `active_group_ids.len()`.
    pub active_count: usize,
}

/// `POST /streams/validate` — decorative live-validation endpoint anchored
/// at the plan's "Validation strategy" note: server is authoritative; the
/// frontend may call this for live feedback in modals, but the same check
/// runs inside `POST /streams`.
///
/// Returns 200 with `{valid: true}` on success, 400
/// `invalid_stream_config` with `reasons: Vec<ValidationIssue>` on failure.
#[utoipa::path(
    post,
    path = "/streams/validate",
    tag = "streams",
    request_body = StreamValidateRequest,
    responses(
        (status = 200, description = "Profile passes encoding-config bounds.", body = StreamValidateResponse),
        (status = 400, description = "Encoding-config bound check failed.", body = ApiErrorBody),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_streams_validate(
    State(_state): State<AppState>,
    axum::Json(req): axum::Json<StreamValidateRequest>,
) -> Result<Json<StreamValidateResponse>, crate::ApiError> {
    spiritstream_core::services::FFmpegHandler::validate_config(&req.profile)?;
    Ok(Json(StreamValidateResponse { valid: true }))
}

/// `GET /streams` — snapshot of which output groups are currently streaming.
/// The full real-time stats path stays on the WebSocket; this endpoint is
/// the typed REST shape callers reach for when they only need "is anything
/// running right now."
#[utoipa::path(
    get,
    path = "/streams",
    tag = "streams",
    responses(
        (status = 200, description = "Active stream snapshot.", body = StreamStatusResponse),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_streams_status(State(state): State<AppState>) -> Json<StreamStatusResponse> {
    let active_group_ids = state.ffmpeg_handler.get_active_group_ids();
    let active_count = state.ffmpeg_handler.active_count();
    Json(StreamStatusResponse {
        active_group_ids,
        active_count,
    })
}

// ---------------------------------------------------------------------------
// Stream lifecycle — typed REST handlers.
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StreamStartRequest {
    /// Output group to start. Runtime type is the core `OutputGroup`;
    /// OpenAPI schema is `OutputGroupWire`.
    #[schema(value_type = OutputGroupWire)]
    pub group: spiritstream_core::models::OutputGroup,
    pub incoming_url: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StreamStartResponse {
    pub pid: u32,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StreamStartAllRequest {
    /// Output groups to start. Runtime is `Vec<OutputGroup>`; OpenAPI
    /// schema is `Vec<OutputGroupWire>`.
    #[schema(value_type = Vec<OutputGroupWire>)]
    pub groups: Vec<spiritstream_core::models::OutputGroup>,
    pub incoming_url: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StreamStartAllResponse {
    pub pids: Vec<u32>,
    /// Group ids core actually started (eligibility is decided
    /// server-side). Clients set their active state from this list
    /// instead of inferring it from what they sent.
    pub started_group_ids: Vec<String>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StreamStopAllResponse {
    pub stopped: bool,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StreamRetryResponse {
    pub pid: u32,
    pub next_delay_secs: Option<u64>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StreamToggleTargetRequest {
    pub enabled: bool,
    /// Output group context for the target toggle. Runtime is core
    /// `OutputGroup`; OpenAPI schema is `OutputGroupWire`.
    #[schema(value_type = OutputGroupWire)]
    pub group: spiritstream_core::models::OutputGroup,
    pub incoming_url: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StreamToggleTargetResponse {
    pub pid: u32,
}

/// `POST /streams/groups/{group_id}` — start streaming for a single output
/// group. The transport layer wires the chat auto-connect side effects via
/// Delegates auto-connect-chat-platforms + start-log-session helpers
/// from the chat module so the start-stream flow brings up chat in
/// the same call.
#[utoipa::path(
    post,
    path = "/streams/groups/{group_id}",
    tag = "streams",
    params(("group_id" = String, Path, description = "Output group ID")),
    request_body = StreamStartRequest,
    responses(
        (status = 200, description = "FFmpeg started.", body = StreamStartResponse),
        (status = 400, description = "Validation failure.", body = ApiErrorBody),
        (status = 422, description = "FFmpeg binary missing or encoder unavailable.", body = ApiErrorBody),
        (status = 500, description = "Spawn error.", body = ApiErrorBody),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_streams_start(
    State(state): State<AppState>,
    axum::extract::Path(_group_id): axum::extract::Path<String>,
    axum::Json(req): axum::Json<StreamStartRequest>,
) -> Result<Json<StreamStartResponse>, crate::ApiError> {
    let was_streaming = state.ffmpeg_handler.active_count() > 0;
    let event_sink: std::sync::Arc<dyn spiritstream_core::services::EventSink> =
        std::sync::Arc::new(state.event_bus.clone());
    // `start` clears the group's retry budget itself (manual start =
    // fresh budget).
    let pid = state
        .ffmpeg_handler
        .start(&req.group, &req.incoming_url, event_sink)?;
    if !was_streaming {
        // Chat (and its always-on encrypted history) is decoupled from
        // streaming — the log session is started once at boot, not here.
        tokio::spawn(crate::auto_connect_chat_platforms(state.clone(), false));
    }
    Ok(Json(StreamStartResponse { pid }))
}

/// `POST /streams` — start every output group with at least one enabled
/// target. Same chat-side-effect orchestration as the single-group form.
#[utoipa::path(
    post,
    path = "/streams",
    tag = "streams",
    request_body = StreamStartAllRequest,
    responses(
        (status = 200, description = "FFmpegs started.", body = StreamStartAllResponse),
        (status = 400, description = "Validation failure.", body = ApiErrorBody),
        (status = 422, description = "FFmpeg binary missing or encoder unavailable.", body = ApiErrorBody),
        (status = 500, description = "Spawn error.", body = ApiErrorBody),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_streams_start_all(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<StreamStartAllRequest>,
) -> Result<Json<StreamStartAllResponse>, crate::ApiError> {
    let event_sink: std::sync::Arc<dyn spiritstream_core::services::EventSink> =
        std::sync::Arc::new(state.event_bus.clone());
    let started = state
        .ffmpeg_handler
        .start_all(&req.groups, &req.incoming_url, event_sink)?;
    tokio::spawn(crate::auto_connect_chat_platforms(state.clone(), false));
    let (started_group_ids, pids) = started.into_iter().unzip();
    Ok(Json(StreamStartAllResponse {
        pids,
        started_group_ids,
    }))
}

/// `DELETE /streams/groups/{group_id}` — stop a single group. Triggers
/// chat auto-disconnect when no groups remain.
#[utoipa::path(
    delete,
    path = "/streams/groups/{group_id}",
    tag = "streams",
    params(("group_id" = String, Path, description = "Output group ID")),
    responses(
        (status = 200, description = "Stopped."),
        (status = 500, body = ApiErrorBody, description = "Internal error during stop."),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_streams_stop(
    State(state): State<AppState>,
    axum::extract::Path(group_id): axum::extract::Path<String>,
) -> Result<Json<StreamStopAllResponse>, crate::ApiError> {
    // I3: `stop_child` polls the FFmpeg child with a 100 ms sleep up to
    // 2 s before SIGKILL. Run on the blocking pool so the async runtime
    // thread isn't parked waiting for graceful shutdown.
    let ffmpeg = state.ffmpeg_handler.clone();
    let gid = group_id.clone();
    tokio::task::spawn_blocking(move || ffmpeg.stop(&gid))
        .await
        .map_err(|e| spiritstream_core::CoreError::Internal {
            context: format!("ffmpeg stop join: {e}"),
        })??;
    // Chat + its always-on encrypted history are decoupled from
    // streaming — stopping a stream no longer ends the log session.
    Ok(Json(StreamStopAllResponse { stopped: true }))
}

/// `DELETE /streams` — stop every group and disconnect chat.
#[utoipa::path(
    delete,
    path = "/streams",
    tag = "streams",
    responses(
        (status = 200, description = "All stopped.", body = StreamStopAllResponse),
        (status = 500, body = ApiErrorBody, description = "Internal error during stop."),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_streams_stop_all(
    State(state): State<AppState>,
) -> Result<Json<StreamStopAllResponse>, crate::ApiError> {
    // I3: graceful-shutdown poll loop runs per group — keep the async
    // runtime free. Chat history is always-on (stream-decoupled), so the
    // log session is NOT ended here.
    let ffmpeg = state.ffmpeg_handler.clone();
    tokio::task::spawn_blocking(move || ffmpeg.stop_all())
        .await
        .map_err(|e| spiritstream_core::CoreError::Internal {
            context: format!("ffmpeg stop_all join: {e}"),
        })??;
    // Chat stays connected after streams stop (decoupled lifecycle).
    Ok(Json(StreamStopAllResponse { stopped: true }))
}

/// `POST /streams/groups/{group_id}/retry` — exponential-backoff reconnect.
#[utoipa::path(
    post,
    path = "/streams/groups/{group_id}/retry",
    tag = "streams",
    params(("group_id" = String, Path, description = "Output group ID")),
    responses(
        (status = 200, description = "Retry kicked off.", body = StreamRetryResponse),
        (status = 400, description = "Validation: group not in active set / already streaming / retries exhausted.", body = ApiErrorBody),
        (status = 422, description = "FFmpeg binary missing or encoder unavailable.", body = ApiErrorBody),
        (status = 500, description = "Spawn error.", body = ApiErrorBody),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_streams_retry(
    State(state): State<AppState>,
    axum::extract::Path(group_id): axum::extract::Path<String>,
) -> Result<Json<StreamRetryResponse>, crate::ApiError> {
    let event_sink: std::sync::Arc<dyn spiritstream_core::services::EventSink> =
        std::sync::Arc::new(state.event_bus.clone());
    let ffmpeg_handler = state.ffmpeg_handler.clone();
    let (pid, next_delay) =
        tokio::task::spawn_blocking(move || ffmpeg_handler.retry_group(&group_id, event_sink))
            .await??;
    Ok(Json(StreamRetryResponse {
        pid,
        next_delay_secs: next_delay.map(|d| d.as_secs()),
    }))
}

/// `PATCH /streams/targets/{target_id}` — toggle an individual target enable
/// state and restart its parent group with the updated filter. PATCH (not
/// PUT) because the body is a partial update — `enabled` flag plus the
/// group context needed to restart, not a full replacement of the target.
#[utoipa::path(
    patch,
    path = "/streams/targets/{target_id}",
    tag = "streams",
    params(("target_id" = String, Path, description = "Target ID")),
    request_body = StreamToggleTargetRequest,
    responses(
        (status = 200, description = "Target toggled, group restarted.", body = StreamToggleTargetResponse),
        (status = 400, description = "Validation failure.", body = ApiErrorBody),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_streams_toggle_target(
    State(state): State<AppState>,
    axum::extract::Path(target_id): axum::extract::Path<String>,
    axum::Json(req): axum::Json<StreamToggleTargetRequest>,
) -> Result<Json<StreamToggleTargetResponse>, crate::ApiError> {
    if req.enabled {
        state.ffmpeg_handler.enable_target(&target_id);
    } else {
        state.ffmpeg_handler.disable_target(&target_id);
    }
    let event_sink: std::sync::Arc<dyn spiritstream_core::services::EventSink> =
        std::sync::Arc::new(state.event_bus.clone());
    let pid = state.ffmpeg_handler.restart_group(
        &req.group.id,
        &req.group,
        &req.incoming_url,
        event_sink,
    )?;
    Ok(Json(StreamToggleTargetResponse { pid }))
}

// ---------------------------------------------------------------------------

// --------------------------------------------------------------------------
// Stream extras.

/// `{"disabled": bool}` — single-flag probe used by the UI to render the
/// per-target enable/disable toggle.
#[derive(serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct StreamTargetDisabledResponse {
    pub disabled: bool,
}

#[utoipa::path(get, path = "/streams/targets/{target_id}/disabled", tag = "streams",
    params(("target_id" = String, Path, description = "Stream target ID")),
    responses((status = 200, body = StreamTargetDisabledResponse)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_stream_target_disabled_proxy(
    State(state): State<AppState>,
    axum::extract::Path(target_id): axum::extract::Path<String>,
) -> Result<Json<StreamTargetDisabledResponse>, crate::ApiError> {
    Ok(Json(StreamTargetDisabledResponse {
        disabled: state.ffmpeg_handler.is_target_disabled(&target_id),
    }))
}
