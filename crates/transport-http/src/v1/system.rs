//! System handlers — `/api/v1/system/*` and `/api/v1/security/*`.
//!
//! Encoder preset matrix, app metadata, client config bounds,
//! FFmpeg discovery / test / update / validate, RTMP test, logs
//! query + export, and the machine-key rotation surface.

use axum::{
    extract::{Query as AxumQuery, State},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use spiritstream_core::services::EventSink;

use crate::AppState;

// ---------------------------------------------------------------------------
// System metadata replacing hardcoded frontend constants.
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EncoderPresetsResponse {
    pub resolutions: Vec<String>,
    pub fps_values: Vec<String>,
    pub audio_bitrates: Vec<String>,
    pub audio_channels: Vec<String>,
    pub audio_sample_rates: Vec<String>,
    pub container_formats: Vec<String>,
    pub h264_profiles: Vec<String>,
    /// Map of encoder kind → preset list. Keys: `libx264`, `libx265`,
    /// `nvenc`, `amf`. Frontend maps the user's selected codec to the
    /// matching list.
    pub presets: std::collections::HashMap<String, Vec<String>>,
    /// Default preset per encoder kind (same key set as `presets`).
    /// Frontend used to derive this by substring-matching the codec
    /// name (`includes('nvenc')` etc.) — that mapping now ships from
    /// the backend so the codec→family relationship is one constant
    /// to edit, not a regex-shaped lookup duplicated client-side.
    pub default_presets: std::collections::HashMap<String, String>,
}

/// `GET /system/encoders/presets` — replaces hardcoded
/// `OutputGroupModal.tsx:22-111` constants. Returned once on app start,
/// cached by the frontend.
#[utoipa::path(
    get,
    path = "/system/encoders/presets",
    tag = "system",
    responses(
        (status = 200, description = "Encoder preset matrix.", body = EncoderPresetsResponse),
    ),
)]
pub async fn v1_system_encoder_presets() -> Json<EncoderPresetsResponse> {
    use std::collections::HashMap;
    let mut presets = HashMap::new();
    presets.insert(
        "libx264".into(),
        vec![
            "ultrafast",
            "superfast",
            "veryfast",
            "faster",
            "fast",
            "medium",
            "slow",
            "slower",
            "veryslow",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
    );
    presets.insert(
        "libx265".into(),
        vec![
            "ultrafast",
            "superfast",
            "veryfast",
            "faster",
            "fast",
            "medium",
            "slow",
            "slower",
            "veryslow",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
    );
    presets.insert(
        "nvenc".into(),
        vec!["p1", "p2", "p3", "p4", "p5", "p6", "p7"]
            .into_iter()
            .map(String::from)
            .collect(),
    );
    presets.insert(
        "amf".into(),
        vec!["quality", "balanced", "speed"]
            .into_iter()
            .map(String::from)
            .collect(),
    );

    let mut default_presets = HashMap::new();
    default_presets.insert("libx264".into(), "veryfast".into());
    default_presets.insert("libx265".into(), "veryfast".into());
    default_presets.insert("nvenc".into(), "p4".into());
    default_presets.insert("amf".into(), "balanced".into());

    Json(EncoderPresetsResponse {
        resolutions: vec!["1920x1080", "1280x720", "2560x1440", "3840x2160", "854x480"]
            .into_iter()
            .map(String::from)
            .collect(),
        fps_values: vec!["60", "30", "24", "25", "50"]
            .into_iter()
            .map(String::from)
            .collect(),
        audio_bitrates: vec!["320k", "256k", "192k", "160k", "128k", "96k", "64k"]
            .into_iter()
            .map(String::from)
            .collect(),
        audio_channels: vec!["1", "2", "6", "8"]
            .into_iter()
            .map(String::from)
            .collect(),
        audio_sample_rates: vec!["48000", "44100", "32000"]
            .into_iter()
            .map(String::from)
            .collect(),
        container_formats: vec!["flv", "mpegts", "mp4"]
            .into_iter()
            .map(String::from)
            .collect(),
        h264_profiles: vec!["baseline", "main", "high"]
            .into_iter()
            .map(String::from)
            .collect(),
        presets,
        default_presets,
    })
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClientConfigResponse {
    /// Delay before triggering SpiritStream when OBS starts streaming (ms).
    pub obs_trigger_delay_ms: u32,
    /// Auto-save debounce delay (ms).
    pub auto_save_delay_ms: u32,
    /// Polling interval for refreshing chat platform status (ms).
    pub chat_poll_interval_ms: u32,
    /// Default toast duration (ms).
    pub toast_duration_ms: u32,
    /// Chat overlay popup window dimensions.
    pub chat_popup_width: u32,
    pub chat_popup_height: u32,
    /// Polling interval for chat overlay state (ms).
    pub chat_overlay_poll_ms: u32,
    /// Base delay for HTTP retry with exponential back-off (ms).
    pub retry_base_delay_ms: u32,
    /// Hard timeout for theme initialization on app start (ms). After this,
    /// the UI falls back to the default theme rather than blocking forever.
    pub theme_init_timeout_ms: u32,
    /// Delay between theme-token fetch retry attempts (ms).
    pub theme_token_retry_delay_ms: u32,
    /// Per-platform chat message max length (already exposed via
    /// `ChatPlatform::max_message_chars`, mirrored here for convenience).
    pub chat_max_chars: std::collections::HashMap<String, u32>,
    /// Video bitrate bounds in kbps. Server-authoritative — frontend
    /// modal renders these for live feedback only; the same range is
    /// enforced by `StreamService::validate_config` on save.
    pub bitrate_range: RangeU32,
    /// Keyframe interval bounds in seconds.
    pub keyframe_range: RangeU32,
    /// Frames-per-second bounds.
    pub fps_range: RangeU32,
    /// Allowed URL prefixes for Discord webhooks. Server-authoritative;
    /// frontend renders the live-validation indicator from this list
    /// rather than hard-coding `discord.com` / `discordapp.com`.
    pub discord_webhook_prefixes: Vec<String>,
    /// Minimum length for the profile-encryption password. Sourced from
    /// `spiritstream_core::services::encryption::PROFILE_PASSWORD_MIN_LENGTH`;
    /// the frontend uses this for inline form feedback only. The server
    /// is authoritative and rejects shorter passwords with
    /// `CoreError::PasswordTooShort` regardless of what the frontend
    /// accepts.
    pub password_min_length: u32,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RangeU32 {
    pub min: u32,
    pub max: u32,
}

/// Response for `GET /system/app-version` — the running app's version
/// string, sourced from Cargo metadata at compile time via `env!`.
/// Replaces the hardcoded `"0.1.0"` constant that drifted out of sync
/// with `package.json` / `tauri.conf.json` / `Cargo.toml`. The About
/// page reads this so bug reports name the actual running version.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppVersionResponse {
    pub version: String,
}

#[utoipa::path(
    get,
    path = "/system/app-version",
    tag = "system",
    responses(
        (status = 200, description = "Running app version (semver).", body = AppVersionResponse),
    ),
)]
pub async fn v1_system_app_version() -> Json<AppVersionResponse> {
    // `CARGO_PKG_VERSION` is the version field from `server/Cargo.toml`
    // (the binary that owns this route). It's the single source of truth
    // — the release workflow already validates the same string across
    // `package.json` / `tauri.conf.json` / desktop+server Cargo.toml.
    Json(AppVersionResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Body for `POST /system/audit/app-update-failure` — frontend records
/// any signature-verification or download error from the self-updater
/// into the HMAC-chained audit log. Operators grep
/// `app_update_signature_failed` in the chain to spot tampered-update
/// attempts that the client-side updater caught.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateFailureRequest {
    /// Sanitized error message from the updater. The frontend strips
    /// anything sensitive before posting; this field is appended verbatim
    /// to the audit entry's `detail`.
    pub detail: String,
}

#[utoipa::path(
    post,
    path = "/system/audit/app-update-failure",
    tag = "system",
    request_body = AppUpdateFailureRequest,
    responses((status = 200, description = "Failure recorded in audit chain.")),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_system_audit_app_update_failure(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<AppUpdateFailureRequest>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    state
        .audit
        .record(spiritstream_core::services::AuditAction::AppUpdateSignatureFailed {
            detail: req.detail,
        })?;
    Ok(Json(serde_json::json!({ "recorded": true })))
}

/// `GET /system/client-config` — replaces hardcoded `apps/web/src/lib/constants.ts`.
/// Returned once on app start, cached by the frontend.
#[utoipa::path(
    get,
    path = "/system/client-config",
    tag = "system",
    responses(
        (status = 200, description = "Server-tuned client constants.", body = ClientConfigResponse),
    ),
)]
pub async fn v1_system_client_config() -> Json<ClientConfigResponse> {
    use spiritstream_core::models::ChatPlatform;
    let mut chat_max_chars = std::collections::HashMap::new();
    for p in [
        ChatPlatform::Twitch,
        ChatPlatform::YouTube,
        ChatPlatform::Trovo,
        ChatPlatform::Kick,
        ChatPlatform::Facebook,
        ChatPlatform::TikTok,
        ChatPlatform::Stripchat,
    ] {
        chat_max_chars.insert(p.as_str().to_owned(), p.max_message_chars() as u32);
    }
    Json(ClientConfigResponse {
        obs_trigger_delay_ms: 2_000,
        auto_save_delay_ms: 500,
        chat_poll_interval_ms: 5_000,
        toast_duration_ms: 4_000,
        chat_popup_width: 420,
        chat_popup_height: 720,
        chat_overlay_poll_ms: 500,
        retry_base_delay_ms: 800,
        theme_init_timeout_ms: 10_000,
        theme_token_retry_delay_ms: 500,
        chat_max_chars,
        // Plan-pinned bounds. The same ranges are enforced by
        // `StreamService::validate_config` on profile save — these
        // values exist for the frontend to render live feedback only.
        bitrate_range: RangeU32 {
            min: 500,
            max: 50_000,
        },
        keyframe_range: RangeU32 { min: 1, max: 10 },
        fps_range: RangeU32 { min: 1, max: 240 },
        discord_webhook_prefixes: vec![
            "https://discord.com/api/webhooks/".to_string(),
            "https://discordapp.com/api/webhooks/".to_string(),
        ],
        password_min_length:
            spiritstream_core::services::PROFILE_PASSWORD_MIN_LENGTH as u32,
    })
}

#[cfg(test)]
mod tests {
    use crate::v1::ApiDoc;
    use utoipa::OpenApi;

    /// The OpenAPI document must be well-formed and contain every typed path
    /// the v1 router registers — this is what `@hey-api/openapi-ts` and any
    /// other API-aware client consumes.
    #[test]
    fn openapi_doc_includes_registered_paths() {
        let doc = ApiDoc::openapi();
        let json = serde_json::to_value(&doc).expect("openapi serializes");

        // OpenAPI 3.0.3 today (see serve_openapi doc comment).
        let version = json
            .get("openapi")
            .and_then(|v| v.as_str())
            .expect("openapi version");
        assert!(
            version.starts_with("3.0") || version.starts_with("3.1"),
            "unexpected openapi version: {version}"
        );

        let paths = json
            .get("paths")
            .and_then(|v| v.as_object())
            .expect("paths object");
        for path in ["/health", "/ready", "/profiles"] {
            assert!(
                paths.contains_key(path),
                "path missing from openapi doc: {path}"
            );
        }
    }

    // `v1_health` now reads AppState to produce a per-subsystem status
    // report. The unit-level smoke test that called it
    // without a state was removed; the typed shape is exercised by
    // `versioned_health_responds_ok` and `health_reports_per_service_status`
    // in `tests/http_surface.rs`.
}

// ===========================================================================
// Typed REST handlers for every command.
//
// The public `POST /api/v1/invoke/:command` URL is gone. The
// `invoke_command` megafunction has been deleted; each handler below
// inlines its own logic directly against the relevant core service and
// any shared helpers in `crate::*`.
// ===========================================================================

/// Discovery endpoint: always 200, returning whatever encoders the local
/// FFmpeg install exposes. When FFmpeg is missing the response is the empty
/// default (the honest answer to "what's available?"). Use
/// `/system/ffmpeg/test` to check whether FFmpeg itself is installed.
#[utoipa::path(get, path = "/system/encoders", tag = "system",
    responses(
        (status = 200, description = "Detected encoders (empty when FFmpeg missing).", body = serde_json::Value),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_system_encoders_proxy(
    State(_state): State<AppState>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    let encoders = spiritstream_core::commands::get_encoders()?;
    Ok(Json(serde_json::json!(encoders)))
}

#[utoipa::path(get, path = "/system/ffmpeg/test", tag = "system",
    responses((status = 200, description = "FFmpeg version string.", body = serde_json::Value)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_system_ffmpeg_test_proxy(
    State(_state): State<AppState>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    let version = spiritstream_core::commands::test_ffmpeg()?;
    Ok(Json(serde_json::json!(version)))
}

#[utoipa::path(get, path = "/system/ffmpeg/path", tag = "system",
    responses((status = 200, description = "Resolved FFmpeg path or null.", body = serde_json::Value)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_system_ffmpeg_path_proxy(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    use spiritstream_core::services::FFmpegLocator;
    let path = FFmpegLocator::discover(Some(&state.settings_manager));
    Ok(Json(serde_json::json!(
        path.map(|p| p.to_string_lossy().to_string())
    )))
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FFmpegUpdateQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_version: Option<String>,
}

#[utoipa::path(get, path = "/system/ffmpeg/update", tag = "system",
    params(("installedVersion" = Option<String>, Query, description = "Currently installed FFmpeg version")),
    responses((status = 200, body = serde_json::Value)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_system_ffmpeg_update_proxy(
    State(state): State<AppState>,
    AxumQuery(q): AxumQuery<FFmpegUpdateQuery>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    // Auto-detect the installed version by running `-version` on the
    // discovered binary when the client doesn't pass it explicitly —
    // frontend never has to learn the bundled-sidecar path itself.
    let detected;
    let installed = match q.installed_version.as_deref() {
        Some(v) if !v.is_empty() => Some(v),
        _ => {
            detected = state
                .ffmpeg_locator
                .detect_installed_version(Some(&state.settings_manager));
            detected.as_deref()
        }
    };
    let info = state.ffmpeg_locator.check_version_status(installed).await;
    Ok(Json(serde_json::json!(info)))
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FFmpegValidatePathRequest {
    pub path: String,
}

#[utoipa::path(post, path = "/system/ffmpeg/validate-path", tag = "system",
    request_body = FFmpegValidatePathRequest,
    responses((status = 200, body = serde_json::Value)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_system_ffmpeg_validate_proxy(
    State(_state): State<AppState>,
    axum::Json(req): axum::Json<FFmpegValidatePathRequest>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    let validated = spiritstream_core::commands::validate_ffmpeg_path(req.path)?;
    Ok(Json(serde_json::json!(validated)))
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RtmpTestRequest {
    pub url: String,
    pub stream_key: String,
}

#[utoipa::path(post, path = "/system/rtmp/test", tag = "system",
    request_body = RtmpTestRequest,
    responses((status = 200, body = serde_json::Value)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_system_rtmp_test_proxy(
    State(_state): State<AppState>,
    axum::Json(req): axum::Json<RtmpTestRequest>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    let result = spiritstream_core::commands::test_rtmp_target(req.url, req.stream_key)?;
    Ok(Json(serde_json::json!(result)))
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LogsQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_lines: Option<usize>,
}

#[utoipa::path(get, path = "/system/logs", tag = "system",
    params(("maxLines" = Option<usize>, Query, description = "Maximum log lines to return")),
    responses(
        (status = 200, body = serde_json::Value, description = "Recent log lines."),
        (status = 500, body = ApiErrorBody, description = "Internal error reading log file."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_system_logs_proxy(
    State(state): State<AppState>,
    AxumQuery(q): AxumQuery<LogsQuery>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    let lines =
        spiritstream_core::services::read_recent_logs(&state.log_dir, q.max_lines.unwrap_or(500))?;
    Ok(Json(serde_json::json!(lines)))
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LogsExportRequest {
    pub path: String,
    pub content: String,
}

#[utoipa::path(post, path = "/system/logs/export", tag = "system",
    request_body = LogsExportRequest,
    responses((status = 200), (status = 403, body = ApiErrorBody)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_system_logs_export_proxy(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<LogsExportRequest>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    let export_path = std::path::PathBuf::from(&req.path);
    let mut allowed_dirs: Vec<&std::path::Path> = vec![state.app_data_dir.as_path()];
    if let Some(ref home) = state.home_dir {
        allowed_dirs.push(home.as_path());
    }
    spiritstream_core::services::validate_path_within_any(&export_path, &allowed_dirs)?;
    std::fs::write(&req.path, &req.content).map_err(|e| {
        spiritstream_core::CoreError::Internal {
            context: format!("Failed to write log file: {e}"),
        }
    })?;
    Ok(Json(serde_json::Value::Null))
}

#[derive(Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct RotateMachineKeyRequest {
    /// Password for each password-protected (`.mgs`) profile on disk, keyed
    /// by profile name. Every `.mgs` profile present must have an entry;
    /// rotation refuses to start otherwise. Body may be omitted entirely
    /// when there are no encrypted profiles.
    #[serde(default)]
    pub unlocked_passwords: std::collections::HashMap<String, String>,
}

#[utoipa::path(post, path = "/security/machine-key/rotate", tag = "security",
    request_body = RotateMachineKeyRequest,
    responses(
        (status = 200, body = serde_json::Value, description = "Rotation report."),
        (status = 400, body = ApiErrorBody, description = "Missing or wrong password for an encrypted profile."),
        (status = 500, body = ApiErrorBody, description = "Rotation aborted; backup restored."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_security_rotate_machine_key_proxy(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    body: Option<Json<RotateMachineKeyRequest>>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    crate::require_confirm_token(&state, &headers, "rotate_machine_key")?;
    let req = body.map(|Json(r)| r).unwrap_or_default();
    let profiles_dir = state.app_data_dir.join("profiles");
    let report = spiritstream_core::services::Encryption::rotate_machine_key(
        &state.app_data_dir,
        &profiles_dir,
        &req.unlocked_passwords,
    )?;
    // Cache invalidation — every cached active-profile field was read under
    // the OLD machine key. Clear them so the next request reloads from disk
    // and surfaces any latent rotation issue immediately rather than at
    // session timeout.
    *state.active_profile_settings.lock().await = None;
    *state.active_profile_name.lock().await = None;
    *state.active_profile_pii.lock().await = None;
    state
        .event_bus
        .emit("active_profile_invalidated", serde_json::json!({}));
    Ok(Json(serde_json::json!(report)))
}
