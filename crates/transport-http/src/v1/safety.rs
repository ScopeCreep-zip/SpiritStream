//! Safety handler — `/api/v1/safety/panic`.

use axum::{extract::State, Json};
use serde::Serialize;
use utoipa::ToSchema;

use crate::AppState;

// ---------------------------------------------------------------------------
// Safety — panic disconnect.
// ---------------------------------------------------------------------------

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SafetyPanicResponse {
    /// Number of active streams that were stopped by the panic.
    pub streams_stopped: usize,
    /// Wall-clock duration of the panic flow, in milliseconds.
    pub elapsed_ms: u64,
}

/// `POST /api/v1/safety/panic` — trigger the panic-disconnect flow.
///
/// Coordinates: stop every active stream, disconnect every chat
/// platform, disconnect OBS, wipe in-memory secret caches, record an
/// audit-log entry, emit `panic_triggered`. See
/// [`spiritstream_core::services::SafetyService`] for the contract.
///
/// **No confirmation token is required** — that defeats the purpose of
/// a panic button. The rate limiter applies (`default_auth`) so a
/// malicious script can't burn the panic call to mask real intent.
#[utoipa::path(
    post,
    path = "/safety/panic",
    tag = "safety",
    responses(
        (status = 200, description = "Panic completed.", body = SafetyPanicResponse),
        (status = 500, description = "Internal error.", body = ApiErrorBody),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_safety_panic(
    State(state): State<AppState>,
) -> Result<Json<SafetyPanicResponse>, crate::ApiError> {
    let svc = state.safety.clone();
    let result = svc.panic().await?;
    Ok(Json(SafetyPanicResponse {
        streams_stopped: result.streams_stopped,
        elapsed_ms: result.elapsed_ms,
    }))
}
