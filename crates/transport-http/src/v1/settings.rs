//! Settings handlers — `/api/v1/settings/*`.
//!
//! Get / save / profiles-path / export / clear-data. Thin shims
//! over `SettingsManager` and `ProfileManager`.

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use spiritstream_core::services::EventSink;

use crate::AppState;

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
