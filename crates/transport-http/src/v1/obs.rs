//! OBS WebSocket handlers — `/api/v1/obs/*`.
//!
//! Connection lifecycle, state snapshot, and the OBS stream start/stop pair.
//! Thin shims over `ObsWebSocketHandler`. OBS settings are NOT read or written
//! here — the active profile is the single source of truth; the handler is
//! synced from it on activation / save (`apply_profile_obs`).

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use spiritstream_core::services::{ObsConnectionStatus, ObsState, ObsStreamStatus};

use crate::AppState;

// --------------------------------------------------------------------------
// Wire-mirror types. utoipa is transport-only, so mirror every core
// payload we hand to / accept from the OBS router instead of leaking
// `ToSchema` into the core crate.

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ObsConnectionStatusWire {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

impl From<ObsConnectionStatus> for ObsConnectionStatusWire {
    fn from(s: ObsConnectionStatus) -> Self {
        match s {
            ObsConnectionStatus::Disconnected => Self::Disconnected,
            ObsConnectionStatus::Connecting => Self::Connecting,
            ObsConnectionStatus::Connected => Self::Connected,
            ObsConnectionStatus::Error => Self::Error,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ObsStreamStatusWire {
    Inactive,
    Starting,
    Active,
    Stopping,
    Unknown,
}

impl From<ObsStreamStatus> for ObsStreamStatusWire {
    fn from(s: ObsStreamStatus) -> Self {
        match s {
            ObsStreamStatus::Inactive => Self::Inactive,
            ObsStreamStatus::Starting => Self::Starting,
            ObsStreamStatus::Active => Self::Active,
            ObsStreamStatus::Stopping => Self::Stopping,
            ObsStreamStatus::Unknown => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ObsStateResponse {
    pub connection_status: ObsConnectionStatusWire,
    pub stream_status: ObsStreamStatusWire,
    pub error_message: Option<String>,
    pub obs_version: Option<String>,
    pub websocket_version: Option<String>,
}

impl From<ObsState> for ObsStateResponse {
    fn from(s: ObsState) -> Self {
        Self {
            connection_status: s.connection_status.into(),
            stream_status: s.stream_status.into(),
            error_message: s.error_message,
            obs_version: s.obs_version,
            websocket_version: s.websocket_version,
        }
    }
}

/// `{"connected": bool}` — single-flag connection probe.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct ObsConnectedResponse {
    pub connected: bool,
}

/// Empty 200 ack body for handlers whose success payload is just
/// acknowledgement (connect / disconnect / set-config / start-stream /
/// stop-stream). Serialises as `{}`.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct ObsAckResponse {}

// --------------------------------------------------------------------------
// OBS handlers.

#[utoipa::path(get, path = "/obs/state", tag = "obs",
    responses((status = 200, body = ObsStateResponse)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_obs_state_proxy(
    State(state): State<AppState>,
) -> Result<Json<ObsStateResponse>, crate::ApiError> {
    let obs_state = state.obs_handler.get_state().await;
    Ok(Json(obs_state.into()))
}

#[utoipa::path(get, path = "/obs/connection", tag = "obs",
    responses((status = 200, body = ObsConnectedResponse)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_obs_is_connected_proxy(
    State(state): State<AppState>,
) -> Result<Json<ObsConnectedResponse>, crate::ApiError> {
    Ok(Json(ObsConnectedResponse {
        connected: state.obs_handler.is_connected().await,
    }))
}

#[utoipa::path(post, path = "/obs/connection", tag = "obs",
    responses(
        (status = 200, body = ObsAckResponse, description = "Connected to OBS WebSocket."),
        (status = 502, body = ApiErrorBody, description = "Network failure reaching OBS."),
        (status = 500, body = ApiErrorBody, description = "Internal error during connect."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_obs_connect_proxy(
    State(state): State<AppState>,
) -> Result<Json<ObsAckResponse>, crate::ApiError> {
    state.obs_handler.connect(state.event_bus.clone()).await?;
    Ok(Json(ObsAckResponse {}))
}

#[utoipa::path(delete, path = "/obs/connection", tag = "obs",
    responses((status = 200, body = ObsAckResponse)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_obs_disconnect_proxy(
    State(state): State<AppState>,
) -> Result<Json<ObsAckResponse>, crate::ApiError> {
    state
        .obs_handler
        .disconnect(state.event_bus.clone())
        .await?;
    Ok(Json(ObsAckResponse {}))
}

#[utoipa::path(post, path = "/obs/stream", tag = "obs",
    responses((status = 200, body = ObsAckResponse)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_obs_start_stream_proxy(
    State(state): State<AppState>,
) -> Result<Json<ObsAckResponse>, crate::ApiError> {
    state.obs_handler.start_stream().await?;
    Ok(Json(ObsAckResponse {}))
}

#[utoipa::path(delete, path = "/obs/stream", tag = "obs",
    responses((status = 200, body = ObsAckResponse)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_obs_stop_stream_proxy(
    State(state): State<AppState>,
) -> Result<Json<ObsAckResponse>, crate::ApiError> {
    state.obs_handler.stop_stream().await?;
    Ok(Json(ObsAckResponse {}))
}
