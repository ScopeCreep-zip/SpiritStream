//! Versioned REST API surface — the *only* HTTP surface SpiritStream serves.
//!
//! Every route this transport exposes lives under `/api/v1/*`. There are no
//! legacy aliases. The single transitional pattern is the
//! `POST /api/v1/invoke/:command` dispatch bridge that is retired one
//! command at a time as typed REST handlers replace each entry.
//!
//! When you add a new typed handler:
//!   1. Annotate it with `#[utoipa::path(...)]`.
//!   2. Register the function in the `ApiDoc::paths(...)` macro below.
//!   3. Register any request/response structs with `components(schemas(...))`.
//!   4. Mount the route in `public_router()` or `protected_router()` per the
//!      auth boundary.
//!
//! See the plan documents for the handler shape, error mapping, and
//! the route mapping table that is progressively filled in.

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use spiritstream_core::services::EventSink;
use utoipa::{OpenApi, ToSchema};

use crate::AppState;

mod chat;
mod discord;
mod oauth;
mod obs;
mod profiles;
mod streams;
mod system;
mod themes;
pub use chat::*;
pub use discord::*;
pub use oauth::*;
pub use obs::*;
pub use profiles::*;
pub use streams::*;
pub use system::*;
pub use themes::*;

/// Aggregated OpenAPI document for the `/api/v1/*` surface.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "SpiritStream HTTP API",
        version = "1.0.0",
        description = "Versioned REST surface for SpiritStream. All clients (web, Tauri desktop, Tauri mobile, CLI, automation) speak this API. See the rewrite plan for the migration shape."
    ),
    servers((url = "/api/v1", description = "API v1")),
    paths(
        v1_health,
        v1_ready,
        v1_profiles_list,
        v1_profile_show,
        v1_profile_save,
        v1_profile_delete,
        v1_profile_is_encrypted,
        v1_settings_get,
        v1_settings_save,
        v1_settings_profiles_path,
        v1_settings_export,
        v1_settings_clear_data,
        v1_safety_panic,
        v1_audit_log,
        v1_streams_validate,
        v1_streams_status,
        v1_streams_start,
        v1_streams_start_all,
        v1_streams_stop,
        v1_streams_stop_all,
        v1_streams_retry,
        v1_streams_toggle_target,
        v1_profile_activate,
        v1_profile_unlock,
        v1_profile_decrypt,
        v1_profile_lock,
        v1_profile_locked_list,
        v1_system_encoder_presets,
        v1_system_app_version,
        v1_system_audit_app_update_failure,
        v1_system_client_config,
        v1_system_encoders_proxy,
        v1_system_ffmpeg_test_proxy,
        v1_system_ffmpeg_path_proxy,
        v1_system_ffmpeg_update_proxy,
        v1_system_ffmpeg_validate_proxy,
        v1_system_rtmp_test_proxy,
        v1_system_logs_proxy,
        v1_system_logs_export_proxy,
        v1_security_rotate_machine_key_proxy,
        v1_profile_summaries_proxy,
        v1_profile_validate_input_proxy,
        v1_profile_order_get_proxy,
        v1_profile_order_set_proxy,
        v1_profile_order_ensure_proxy,
        v1_themes_list_proxy,
        v1_themes_install_proxy,
        v1_themes_refresh_proxy,
        v1_theme_tokens_proxy,
        v1_obs_state_proxy,
        v1_obs_get_config_proxy,
        v1_obs_set_config_proxy,
        v1_obs_is_connected_proxy,
        v1_obs_connect_proxy,
        v1_obs_disconnect_proxy,
        v1_obs_start_stream_proxy,
        v1_obs_stop_stream_proxy,
        v1_discord_test_webhook_proxy,
        v1_discord_send_notification_proxy,
        v1_discord_reset_cooldown_proxy,
        v1_chat_status_proxy,
        v1_chat_connect_proxy,
        v1_chat_disconnect_all_proxy,
        v1_chat_platform_status_proxy,
        v1_chat_disconnect_proxy,
        v1_chat_retry_proxy,
        v1_chat_send_proxy,
        v1_chat_is_connected_proxy,
        v1_chat_log_status_proxy,
        v1_chat_export_log_proxy,
        v1_chat_search_session_proxy,
        v1_oauth_get_config_proxy,
        v1_oauth_set_config_proxy,
        v1_oauth_is_configured_proxy,
        v1_oauth_start_flow_proxy,
        v1_oauth_complete_flow_proxy,
        v1_oauth_get_account_proxy,
        v1_oauth_disconnect_proxy,
        v1_oauth_forget_proxy,
        v1_oauth_refresh_token_proxy,
        v1_stream_target_disabled_proxy,
    ),
    components(schemas(
        HealthResponse,
        ReadyResponse,
        ReadyCheckFailure,
        ProfilesListResponse,
        ProfileShowQuery,
        ProfileSaveRequest,
        ProfileSaveResponse,
        ProfileDeleteResponse,
        ProfileIsEncryptedResponse,
        SettingsSaveRequest,
        SettingsSaveResponse,
        SettingsProfilesPathResponse,
        SettingsExportRequest,
        SettingsExportResponse,
        SettingsClearDataResponse,
        SafetyPanicResponse,
        AuditLogResponse,
        SubsystemStatus,
        StreamValidateRequest,
        StreamValidateResponse,
        StreamStatusResponse,
        StreamStartRequest,
        StreamStartResponse,
        StreamStartAllRequest,
        StreamStartAllResponse,
        StreamStopAllResponse,
        StreamRetryResponse,
        StreamToggleTargetRequest,
        StreamToggleTargetResponse,
        ProfileActivateRequest,
        ProfileUnlockRequest,
        ProfileUnlockResponse,
        ProfileDecryptRequest,
        ProfileDecryptResponse,
        ProfileLockResponse,
        ProfileLockedListResponse,
        EncoderPresetsResponse,
        ClientConfigResponse,
        RangeU32,
        ApiErrorBody,
        RotateMachineKeyRequest,
    ))
)]
pub struct ApiDoc;

/// Build the public `/api/v1/*` sub-router (no auth required).
pub fn public_router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/v1/health", get(v1_health))
        .route("/api/v1/ready", get(v1_ready))
        .route("/api/v1/openapi.json", get(serve_openapi))
        .with_state(state)
}

/// Build the protected `/api/v1/*` sub-router (auth middleware applied by caller).
pub fn protected_router(state: AppState) -> Router<AppState> {
    use axum::routing::{delete, patch, put};
    Router::new()
        // Typed REST handlers — Profiles.
        .route("/api/v1/profiles", get(v1_profiles_list))
        .route("/api/v1/profiles/:name", get(v1_profile_show))
        .route("/api/v1/profiles/:name", put(v1_profile_save))
        .route("/api/v1/profiles/:name", delete(v1_profile_delete))
        .route(
            "/api/v1/profiles/:name/encrypted",
            get(v1_profile_is_encrypted),
        )
        // Settings — typed REST.
        .route(
            "/api/v1/settings",
            get(v1_settings_get).put(v1_settings_save),
        )
        .route(
            "/api/v1/settings/profiles-path",
            get(v1_settings_profiles_path),
        )
        .route("/api/v1/settings/export", post(v1_settings_export))
        .route("/api/v1/settings/data", delete(v1_settings_clear_data))
        // Safety panic.
        .route("/api/v1/safety/panic", post(v1_safety_panic))
        // Audit log read endpoint.
        .route("/api/v1/audit/log", get(v1_audit_log))
        // Streams — typed REST.
        .route("/api/v1/streams", get(v1_streams_status))
        .route("/api/v1/streams", post(v1_streams_start_all))
        .route("/api/v1/streams", delete(v1_streams_stop_all))
        .route("/api/v1/streams/validate", post(v1_streams_validate))
        .route("/api/v1/streams/groups/:group_id", post(v1_streams_start))
        .route("/api/v1/streams/groups/:group_id", delete(v1_streams_stop))
        .route(
            "/api/v1/streams/groups/:group_id/retry",
            post(v1_streams_retry),
        )
        .route(
            "/api/v1/streams/targets/:target_id",
            patch(v1_streams_toggle_target),
        )
        // Profile activation + decrypt + lock — server-side session state.
        .route("/api/v1/profiles/:name/activate", post(v1_profile_activate))
        .route("/api/v1/profiles/:name/unlock", post(v1_profile_unlock))
        .route("/api/v1/profiles/:name/decrypt", post(v1_profile_decrypt))
        .route("/api/v1/profiles/:name/lock", post(v1_profile_lock))
        .route("/api/v1/profiles/locked", get(v1_profile_locked_list))
        // System metadata replacing frontend constants.
        .route(
            "/api/v1/system/encoders/presets",
            get(v1_system_encoder_presets),
        )
        .route("/api/v1/system/app-version", get(v1_system_app_version))
        .route(
            "/api/v1/system/audit/app-update-failure",
            post(v1_system_audit_app_update_failure),
        )
        .route("/api/v1/system/client-config", get(v1_system_client_config))
        // System — FFmpeg + RTMP + logs.
        .route("/api/v1/system/encoders", get(v1_system_encoders_proxy))
        .route(
            "/api/v1/system/ffmpeg/test",
            get(v1_system_ffmpeg_test_proxy),
        )
        .route(
            "/api/v1/system/ffmpeg/path",
            get(v1_system_ffmpeg_path_proxy),
        )
        .route(
            "/api/v1/system/ffmpeg/update",
            get(v1_system_ffmpeg_update_proxy),
        )
        .route(
            "/api/v1/system/ffmpeg/validate-path",
            post(v1_system_ffmpeg_validate_proxy),
        )
        .route("/api/v1/system/rtmp/test", post(v1_system_rtmp_test_proxy))
        .route("/api/v1/system/logs", get(v1_system_logs_proxy))
        .route(
            "/api/v1/system/logs/export",
            post(v1_system_logs_export_proxy),
        )
        // Security — machine key rotation.
        .route(
            "/api/v1/security/machine-key/rotate",
            post(v1_security_rotate_machine_key_proxy),
        )
        // Profiles — remaining typed routes.
        .route(
            "/api/v1/profiles/summaries",
            get(v1_profile_summaries_proxy),
        )
        .route(
            "/api/v1/profiles/validate-input",
            post(v1_profile_validate_input_proxy),
        )
        .route(
            "/api/v1/profiles/order",
            get(v1_profile_order_get_proxy).patch(v1_profile_order_set_proxy),
        )
        .route(
            "/api/v1/profiles/order/ensure",
            post(v1_profile_order_ensure_proxy),
        )
        // Themes.
        .route(
            "/api/v1/themes",
            get(v1_themes_list_proxy).post(v1_themes_install_proxy),
        )
        .route("/api/v1/themes/refresh", post(v1_themes_refresh_proxy))
        .route(
            "/api/v1/themes/:theme_id/tokens",
            get(v1_theme_tokens_proxy),
        )
        // OBS.
        .route("/api/v1/obs/state", get(v1_obs_state_proxy))
        .route(
            "/api/v1/obs/config",
            get(v1_obs_get_config_proxy).put(v1_obs_set_config_proxy),
        )
        .route(
            "/api/v1/obs/connection",
            get(v1_obs_is_connected_proxy)
                .post(v1_obs_connect_proxy)
                .delete(v1_obs_disconnect_proxy),
        )
        .route(
            "/api/v1/obs/stream",
            post(v1_obs_start_stream_proxy).delete(v1_obs_stop_stream_proxy),
        )
        // Discord.
        .route(
            "/api/v1/discord/webhook/test",
            post(v1_discord_test_webhook_proxy),
        )
        .route(
            "/api/v1/discord/webhook/send",
            post(v1_discord_send_notification_proxy),
        )
        .route(
            "/api/v1/discord/webhook/cooldown",
            delete(v1_discord_reset_cooldown_proxy),
        )
        // Chat.
        .route(
            "/api/v1/chat/connections",
            get(v1_chat_status_proxy)
                .post(v1_chat_connect_proxy)
                .delete(v1_chat_disconnect_all_proxy),
        )
        .route(
            "/api/v1/chat/connections/:platform",
            get(v1_chat_platform_status_proxy).delete(v1_chat_disconnect_proxy),
        )
        .route(
            "/api/v1/chat/connections/:platform/retry",
            post(v1_chat_retry_proxy),
        )
        .route("/api/v1/chat/messages", post(v1_chat_send_proxy))
        .route("/api/v1/chat/connected", get(v1_chat_is_connected_proxy))
        .route("/api/v1/chat/log", get(v1_chat_log_status_proxy))
        .route("/api/v1/chat/log/export", post(v1_chat_export_log_proxy))
        .route(
            "/api/v1/chat/log/search",
            post(v1_chat_search_session_proxy),
        )
        // OAuth.
        .route(
            "/api/v1/oauth/config",
            get(v1_oauth_get_config_proxy).put(v1_oauth_set_config_proxy),
        )
        .route(
            "/api/v1/oauth/:provider/configured",
            get(v1_oauth_is_configured_proxy),
        )
        .route(
            "/api/v1/oauth/:provider/flow",
            post(v1_oauth_start_flow_proxy),
        )
        .route(
            "/api/v1/oauth/:provider/complete",
            post(v1_oauth_complete_flow_proxy),
        )
        .route(
            "/api/v1/oauth/:provider/account",
            get(v1_oauth_get_account_proxy).delete(v1_oauth_disconnect_proxy),
        )
        .route(
            "/api/v1/oauth/:provider/forget",
            post(v1_oauth_forget_proxy),
        )
        .route(
            "/api/v1/oauth/:provider/refresh",
            post(v1_oauth_refresh_token_proxy),
        )
        // Stream extras still on the legacy command set.
        .route(
            "/api/v1/streams/targets/:target_id/disabled",
            get(v1_stream_target_disabled_proxy),
        )
        .with_state(state)
}

/// Serve the OpenAPI document describing the `/api/v1/*` surface.
///
/// The current emitter (utoipa 4 with axum 0.7) produces an OpenAPI 3.0.3
/// document. The rewrite plan calls for 3.1, which requires bumping to
/// utoipa 5 + axum 0.8 — that upgrade is folded into a later cleanup pass
/// since the type-generator we use (`@hey-api/openapi-ts`)
/// accepts both 3.0 and 3.1 specs without behavior change.
async fn serve_openapi() -> impl IntoResponse {
    Json(ApiDoc::openapi())
}

// ---------------------------------------------------------------------------
// Health & readiness
// ---------------------------------------------------------------------------

/// Per-subsystem status report.
///
/// Each variant maps cleanly to a UI badge color: `ok` → green,
/// `degraded` → amber, `disconnected`/`tampered` → red. Adding a new
/// variant is a deliberate act — every subsystem reports through this
/// enum so the surface stays bounded.
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum SubsystemStatus {
    /// Fully operational. `detail` is omitted.
    Ok,
    /// Functioning but in a degraded state. `detail` carries the
    /// reason (e.g. `"FFmpeg not on PATH; using bundled binary"`).
    Degraded { detail: String },
    /// Subsystem requires user attention. `detail` describes why.
    Disconnected { detail: String },
    /// Audit-log tamper detection found a chain break. The number is
    /// the last verified sequence so the UI can render the red banner.
    Tampered { last_valid_sequence: u64 },
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    /// Aggregate status string — `"ok"`, `"degraded"`, or `"tampered"`.
    /// Kept for backwards-compat with Tauri sidecar polling + the
    /// Docker `HEALTHCHECK` directive, which just need 200 + a key
    /// they can grep for.
    pub status: String,
    /// Per-subsystem report. Frontend status views render
    /// each entry as its own row with the corresponding badge.
    pub services: std::collections::BTreeMap<String, SubsystemStatus>,
}

#[utoipa::path(
    get,
    path = "/health",
    tag = "system",
    responses(
        (status = 200, description = "Per-subsystem health snapshot.", body = HealthResponse),
    ),
)]
pub async fn v1_health(State(state): State<AppState>) -> Json<HealthResponse> {
    use std::collections::BTreeMap;
    let mut services: BTreeMap<String, SubsystemStatus> = BTreeMap::new();

    // Profile, settings, theme — same checks as `/ready` but
    // surfaced as typed subsystem statuses rather than a flat fail list.
    services.insert(
        "profiles".into(),
        match state.profile_manager.get_all_names().await {
            Ok(_) => SubsystemStatus::Ok,
            Err(err) => SubsystemStatus::Disconnected {
                detail: err.to_string(),
            },
        },
    );
    services.insert(
        "settings".into(),
        match state.settings_manager.load() {
            Ok(_) => SubsystemStatus::Ok,
            Err(err) => SubsystemStatus::Disconnected {
                detail: err.to_string(),
            },
        },
    );
    services.insert(
        "themes".into(),
        if state.theme_manager.list_themes().is_empty() {
            SubsystemStatus::Degraded {
                detail: "no themes loaded".into(),
            }
        } else {
            SubsystemStatus::Ok
        },
    );
    // Audit log chain — surface tamper status to dashboards.
    services.insert(
        "audit_log".into(),
        match state.audit.verify_chain() {
            Ok(spiritstream_core::services::AuditChainStatus::Ok { .. }) => SubsystemStatus::Ok,
            Ok(spiritstream_core::services::AuditChainStatus::Empty) => SubsystemStatus::Ok,
            Ok(spiritstream_core::services::AuditChainStatus::Tampered {
                last_valid_sequence,
                ..
            }) => SubsystemStatus::Tampered {
                last_valid_sequence,
            },
            Err(e) => SubsystemStatus::Disconnected {
                detail: e.to_string(),
            },
        },
    );

    let status = if services
        .values()
        .any(|s| matches!(s, SubsystemStatus::Tampered { .. }))
    {
        "tampered"
    } else if services.values().any(|s| {
        matches!(
            s,
            SubsystemStatus::Disconnected { .. } | SubsystemStatus::Degraded { .. }
        )
    }) {
        "degraded"
    } else {
        "ok"
    };

    Json(HealthResponse {
        status: status.into(),
        services,
    })
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ReadyResponse {
    /// `true` when every critical subsystem reports healthy.
    pub ready: bool,
    /// Names of subsystems that failed their check. Empty when `ready` is true.
    pub failed: Vec<String>,
    /// Per-subsystem failure detail. Empty when `ready` is true.
    pub errors: Vec<ReadyCheckFailure>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ReadyCheckFailure {
    /// Subsystem name (e.g. `"profiles"`, `"settings"`).
    pub check: String,
    /// Short human-readable error from the subsystem.
    pub error: String,
}

/// How long the long-poll holds the connection before returning 503 +
/// `Retry-After`. Tuned well below typical reverse-proxy idle timeouts
/// (Caddy / nginx default 60s+) so the client gets a real response,
/// not a proxy-induced reset.
const READY_LONG_POLL_TIMEOUT_SECS: u64 = 25;

#[utoipa::path(
    get,
    path = "/ready",
    tag = "system",
    responses(
        (status = 200, description = "Server fully initialized and ready to serve requests.", body = ReadyResponse),
        (status = 503, description = "Still initializing; client should retry after the `Retry-After` interval.", body = ReadyResponse),
    ),
)]
pub async fn v1_ready(State(state): State<AppState>) -> impl IntoResponse {
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    let ready_body = || ReadyResponse {
        ready: true,
        failed: Vec::new(),
        errors: Vec::new(),
    };

    // Fast path: already ready. No parking, no awaits.
    if state.readiness.ready.load(Ordering::Acquire) {
        return (StatusCode::OK, Json(ready_body())).into_response();
    }

    // Slow path: park on the Notify until services finish initializing OR
    // the long-poll timeout elapses. Server-side wait eliminates the
    // client-side retry loop that produced "Failed to load resource" noise
    // in the WebKit console during boot.
    let notified = state.readiness.notify.notified();
    let timeout = tokio::time::sleep(Duration::from_secs(READY_LONG_POLL_TIMEOUT_SECS));

    tokio::select! {
        _ = notified => {
            (StatusCode::OK, Json(ready_body())).into_response()
        }
        _ = timeout => {
            let body = ReadyResponse {
                ready: false,
                failed: vec!["timeout".into()],
                errors: vec![ReadyCheckFailure {
                    check: "readiness".into(),
                    error: format!("services did not initialize within {READY_LONG_POLL_TIMEOUT_SECS}s"),
                }],
            };
            let mut response = (StatusCode::SERVICE_UNAVAILABLE, Json(body)).into_response();
            // RFC 9110 — direct the client to wait before retrying.
            response.headers_mut().insert(
                axum::http::header::RETRY_AFTER,
                axum::http::HeaderValue::from_static("1"),
            );
            response
        }
    }
}

// ---------------------------------------------------------------------------
// Settings.
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
pub struct SettingsSaveRequest {
    /// Full settings body (matches the `Settings` ts-rs export).
    pub settings: serde_json::Value,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct SettingsSaveResponse {
    pub saved: bool,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct SettingsProfilesPathResponse {
    pub path: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SettingsExportRequest {
    /// Absolute path of the destination directory. Must resolve inside the
    /// app data dir or the user's home — anything else is rejected with
    /// `path_outside_allowed_root`.
    pub export_path: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct SettingsExportResponse {
    pub exported: bool,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct SettingsClearDataResponse {
    pub cleared: bool,
}

/// `GET /settings` — return the resolved (cached) global settings document.
#[utoipa::path(
    get,
    path = "/settings",
    tag = "settings",
    responses(
        (status = 200, description = "Resolved settings.", body = serde_json::Value),
        (status = 500, description = "Internal error.", body = ApiErrorBody),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_settings_get(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    let settings = state.settings_manager.load()?;
    Ok(Json(serde_json::to_value(settings)?))
}

/// `PUT /settings` — replace the global settings document. Field-level bound
/// checks (`log_retention_days` ∈ [1, 365], `discord_cooldown_seconds`
/// ∈ [0, 86400], `backend_port` ∈ [1, 65535]) run inside
/// `SettingsManager::save`. Out-of-range values return 400
/// `validation_failed` with the full list of offending fields.
#[utoipa::path(
    put,
    path = "/settings",
    tag = "settings",
    request_body = SettingsSaveRequest,
    responses(
        (status = 200, description = "Settings saved.", body = SettingsSaveResponse),
        (status = 400, description = "Bound-check failed.", body = ApiErrorBody),
        (status = 500, description = "Internal error.", body = ApiErrorBody),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_settings_save(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<SettingsSaveRequest>,
) -> Result<Json<SettingsSaveResponse>, crate::ApiError> {
    let new_settings: spiritstream_core::models::Settings = serde_json::from_value(req.settings)
        .map_err(|e| spiritstream_core::CoreError::ValidationFailed {
            reasons: vec![spiritstream_core::errors::ValidationIssue {
                code: "invalid_settings_shape".into(),
                message: format!("could not parse settings body: {e}"),
                path: None,
            }],
        })?;

    state.settings_manager.save(&new_settings)?;

    // Per-profile `encrypt_stream_keys` is enforced inside
    // `ProfileManager::save_with_key_encryption`; flipping the
    // global-settings flag no longer rewrites every profile (that
    // bulk-rewrite would silently fall back through a global toggle
    // we no longer carry — per-profile encrypt-on-save is the
    // forward-only replacement).

    let _ = crate::prune_logs(&state.log_dir, new_settings.log_retention_days);
    state
        .event_bus
        .emit("settings_changed", serde_json::json!({}));

    Ok(Json(SettingsSaveResponse { saved: true }))
}

/// `GET /settings/profiles-path` — return the absolute on-disk path of the
/// profiles directory for the active install. The frontend uses it for the
/// "open profiles folder" affordance.
#[utoipa::path(
    get,
    path = "/settings/profiles-path",
    tag = "settings",
    responses(
        (status = 200, description = "Profiles directory path.", body = SettingsProfilesPathResponse),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_settings_profiles_path(
    State(state): State<AppState>,
) -> Json<SettingsProfilesPathResponse> {
    let path = state.settings_manager.get_profiles_path();
    Json(SettingsProfilesPathResponse {
        path: path.to_string_lossy().to_string(),
    })
}

/// `POST /settings/export` — copy `settings.json` and every profile under
/// `<export_path>/`. The destination must resolve inside the app data dir or
/// the user's home; anything else returns 403 `path_outside_allowed_root`.
#[utoipa::path(
    post,
    path = "/settings/export",
    tag = "settings",
    request_body = SettingsExportRequest,
    responses(
        (status = 200, description = "Export complete.", body = SettingsExportResponse),
        (status = 403, description = "Export path outside allowed roots.", body = ApiErrorBody),
        (status = 500, description = "Internal error.", body = ApiErrorBody),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_settings_export(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<SettingsExportRequest>,
) -> Result<Json<SettingsExportResponse>, crate::ApiError> {
    let path = std::path::PathBuf::from(&req.export_path);

    let mut allowed_dirs: Vec<&std::path::Path> = vec![state.app_data_dir.as_path()];
    if let Some(ref home) = state.home_dir {
        allowed_dirs.push(home.as_path());
    }

    crate::validate_path_within_any(&path, &allowed_dirs)?;

    state.settings_manager.export_data(&path)?;
    Ok(Json(SettingsExportResponse { exported: true }))
}

/// `DELETE /settings/data` — wipe every persisted setting + every profile.
/// A future change will gate this behind a per-call confirmation token;
/// for now the authenticated session is the only gate.
#[utoipa::path(
    delete,
    path = "/settings/data",
    tag = "settings",
    responses(
        (status = 200, description = "Data cleared.", body = SettingsClearDataResponse),
        (status = 500, description = "Internal error.", body = ApiErrorBody),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_settings_clear_data(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<SettingsClearDataResponse>, crate::ApiError> {
    crate::require_confirm_token(&state, &headers, "clear_data")?;
    state.settings_manager.clear_data()?;
    Ok(Json(SettingsClearDataResponse { cleared: true }))
}

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

// ---------------------------------------------------------------------------
// Audit log — read endpoint.
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct AuditLogQuery {
    /// Skip this many leading entries (oldest first). Defaults to 0.
    #[serde(default)]
    skip: usize,
    /// Maximum entries to return. Defaults to 200. Hard-capped at 1000
    /// so a careless client can't OOM the server.
    #[serde(default)]
    limit: Option<usize>,
    /// When set, only entries whose `action.kind` matches this string
    /// are returned. Used to filter by kind
    /// (panic_triggered, chat_message_pii_blocked, oauth_refresh, …).
    #[serde(default)]
    kind: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogResponse {
    /// Total entries that survive the filter (before paging).
    pub total: usize,
    /// Page slice (oldest-first within the returned window).
    pub entries: Vec<serde_json::Value>,
    /// HMAC chain status. The audit-log UI inspects this on
    /// every fetch; a `tampered` value triggers the red banner.
    /// Always computed server-side against the on-disk log;
    /// clients cannot influence it.
    pub chain: serde_json::Value,
}

/// `GET /api/v1/audit/log` — paginated, filterable read of the audit
/// log. Entries are serialised as-is from
/// [`spiritstream_core::services::AuditEntry`]. The response is wrapped
/// in an HMAC-verification status (`tampered: bool` + last known-good
/// sequence) so the UI can render the red banner.
#[utoipa::path(
    get,
    path = "/audit/log",
    tag = "safety",
    params(
        ("skip" = Option<usize>, Query, description = "Skip N entries (oldest first)."),
        ("limit" = Option<usize>, Query, description = "Max entries per page (≤ 1000)."),
        ("kind" = Option<String>, Query, description = "Filter by action kind."),
    ),
    responses(
        (status = 200, description = "Audit entries.", body = AuditLogResponse),
        (status = 500, description = "Internal error.", body = ApiErrorBody),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_audit_log(
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<AuditLogQuery>,
) -> Result<Json<AuditLogResponse>, crate::ApiError> {
    // Verify the HMAC chain before serving entries. We
    // surface the status to the client either way so the UI can show
    // the tamper banner even when the user has filtered the page to
    // an empty result.
    let chain_status = state.audit.verify_chain().unwrap_or_else(|e| {
        spiritstream_core::services::AuditChainStatus::Tampered {
            last_valid_sequence: 0,
            reason: format!("verify error: {e}"),
        }
    });
    let chain = serde_json::to_value(&chain_status).unwrap_or(serde_json::Value::Null);

    let entries = state.audit.entries()?;
    let filtered: Vec<serde_json::Value> = entries
        .into_iter()
        .filter(|e| match &q.kind {
            None => true,
            Some(k) => action_kind_str(&e.action) == k.as_str(),
        })
        .map(|e| serde_json::to_value(&e).unwrap_or(serde_json::Value::Null))
        .collect();
    let total = filtered.len();
    let skip = q.skip.min(total);
    let limit = q.limit.unwrap_or(200).min(1000);
    let page = filtered.into_iter().skip(skip).take(limit).collect();
    Ok(Json(AuditLogResponse {
        total,
        entries: page,
        chain,
    }))
}

fn action_kind_str(action: &spiritstream_core::services::AuditAction) -> &'static str {
    use spiritstream_core::services::AuditAction::*;
    match action {
        PanicTriggered { .. } => "panic_triggered",
        ChatMessagePiiBlocked { .. } => "chat_message_pii_blocked",
        ProfileSaved { .. } => "profile_saved",
        ProfileDeleted { .. } => "profile_deleted",
        OauthRefresh { .. } => "oauth_refresh",
        OauthRefreshUnusualLocation { .. } => "oauth_refresh_unusual_location",
        MachineKeyRotated { .. } => "machine_key_rotated",
        AnonymousModeToggled { .. } => "anonymous_mode_toggled",
        AppStarted => "app_started",
        AppStopped => "app_stopped",
        AuditLogTamperDetected { .. } => "audit_log_tamper_detected",
        ThemeValidationFailed { .. } => "theme_validation_failed",
        AppUpdateSignatureFailed { .. } => "app_update_signature_failed",
        ChatMessageSent { .. } => "chat_message_sent",
        ChatPlatformConnected { .. } => "chat_platform_connected",
        ChatPlatformDisconnected { .. } => "chat_platform_disconnected",
    }
}



