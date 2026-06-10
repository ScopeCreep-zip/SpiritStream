//! OBS WebSocket handlers — `/api/v1/obs/*`.
//!
//! Connection lifecycle, state snapshot, config get/set, and the
//! OBS stream start/stop pair. Thin shims over `ObsWebSocketHandler`.

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use spiritstream_core::services::{
    IntegrationDirection, ObsConfig, ObsConnectionStatus, ObsState, ObsStreamStatus,
};

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
#[serde(rename_all = "kebab-case")]
pub enum IntegrationDirectionWire {
    ObsToSpiritstream,
    SpiritstreamToObs,
    Bidirectional,
    Disabled,
}

impl From<IntegrationDirection> for IntegrationDirectionWire {
    fn from(d: IntegrationDirection) -> Self {
        match d {
            IntegrationDirection::ObsToSpiritstream => Self::ObsToSpiritstream,
            IntegrationDirection::SpiritstreamToObs => Self::SpiritstreamToObs,
            IntegrationDirection::Bidirectional => Self::Bidirectional,
            IntegrationDirection::Disabled => Self::Disabled,
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

/// The OBS password never rides this response — only whether one is
/// set. Clients that need the value already have it from the profile
/// document; shipping it here (the old shape returned the DECRYPTED
/// password) widened the exposure surface for zero benefit.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ObsConfigResponse {
    pub host: String,
    pub port: u16,
    pub has_password: bool,
    pub use_auth: bool,
    pub direction: IntegrationDirectionWire,
    pub auto_connect: bool,
}

impl From<ObsConfig> for ObsConfigResponse {
    fn from(c: ObsConfig) -> Self {
        Self {
            host: c.host,
            port: c.port,
            has_password: !c.password.is_empty(),
            use_auth: c.use_auth,
            direction: c.direction.into(),
            auto_connect: c.auto_connect,
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

#[utoipa::path(get, path = "/obs/config", tag = "obs",
    responses((status = 200, body = ObsConfigResponse)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_obs_get_config_proxy(
    State(state): State<AppState>,
) -> Result<Json<ObsConfigResponse>, crate::ApiError> {
    // No decryption: the response carries `hasPassword`, never the value.
    let config = state.obs_handler.get_config().await;
    Ok(Json(config.into()))
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ObsSetConfigRequest {
    pub host: String,
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    pub use_auth: bool,
    pub direction: String,
    pub auto_connect: bool,
}

#[utoipa::path(put, path = "/obs/config", tag = "obs",
    request_body = ObsSetConfigRequest,
    responses(
        (status = 200, body = ObsAckResponse, description = "OBS config persisted."),
        (status = 500, body = ApiErrorBody, description = "Internal error encrypting password."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_obs_set_config_proxy(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<ObsSetConfigRequest>,
) -> Result<Json<ObsAckResponse>, crate::ApiError> {
    let current_config = state.obs_handler.get_config().await;
    let encrypted_password = if let Some(ref pass) = req.password {
        if pass.is_empty() {
            String::new()
        } else {
            state.obs_handler.encrypt_password(pass)?
        }
    } else {
        current_config.password
    };
    let dir = match req.direction.as_str() {
        "obs-to-spiritstream" => IntegrationDirection::ObsToSpiritstream,
        "spiritstream-to-obs" => IntegrationDirection::SpiritstreamToObs,
        "bidirectional" => IntegrationDirection::Bidirectional,
        _ => IntegrationDirection::Disabled,
    };
    let config = ObsConfig {
        host: req.host,
        port: req.port,
        password: encrypted_password,
        use_auth: req.use_auth,
        direction: dir,
        auto_connect: req.auto_connect,
    };
    state.obs_handler.set_config(config).await;
    Ok(Json(ObsAckResponse {}))
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
