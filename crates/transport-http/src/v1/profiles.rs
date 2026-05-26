//! Profile handlers — `/api/v1/profiles/*`.
//!
//! List + CRUD, activate/unlock/decrypt/lock session-state surface,
//! and the residual profile-summaries / validate-input / order proxies.

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use spiritstream_core::services::EventSink;

use crate::AppState;

// ---------------------------------------------------------------------------
// Profiles — proof-of-concept typed handler. Full CRUD ships later.
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ProfilesListResponse {
    /// Profile names in user-defined order.
    pub names: Vec<String>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ApiErrorBody {
    /// Stable error kind from `spiritstream_core::CoreError`.
    pub kind: String,
    /// Optional human-readable message. Internal details are never included.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[utoipa::path(
    get,
    path = "/profiles",
    tag = "profiles",
    responses(
        (status = 200, description = "List of profile names.", body = ProfilesListResponse),
        (status = 401, description = "Authentication required.", body = ApiErrorBody),
        (status = 500, description = "Internal server error.", body = ApiErrorBody),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_profiles_list(
    State(state): State<AppState>,
) -> Result<Json<ProfilesListResponse>, crate::ApiError> {
    let names = state.profile_manager.get_all_names().await?;
    Ok(Json(ProfilesListResponse { names }))
}

// ---------------------------------------------------------------------------
// Profile resource — typed CRUD.
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ProfileShowQuery {
    /// Password for encrypted profiles. Plaintext profiles ignore this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ProfileDeleteResponse {
    pub name: String,
    pub deleted: bool,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ProfileSaveRequest {
    /// Full profile body (matches the `Profile` ts-rs export).
    pub profile: serde_json::Value,
    /// Optional password — when present the profile is encrypted on disk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ProfileSaveResponse {
    pub name: String,
    pub saved: bool,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ProfileIsEncryptedResponse {
    pub name: String,
    pub encrypted: bool,
}

/// `GET /profiles/{name}` — return the full profile body. `password` is
/// required as a query parameter for encrypted profiles; plaintext profiles
/// ignore it. Encrypted-but-no-password returns 401 `password_required`.
#[utoipa::path(
    get,
    path = "/profiles/{name}",
    tag = "profiles",
    params(
        ("name" = String, Path, description = "Profile name"),
    ),
    responses(
        // Body is the full `Profile` shape — typed by ts-rs at
        // `@spiritstream/types/Profile`. utoipa documents the runtime as
        // a free-form object because the Profile tree (OutputGroup,
        // ProfileSettings, generated Platform enum, …) is too deep to
        // mirror by hand and adding `ToSchema` to core would leak utoipa
        // across the transport boundary. The wire shape is camelCase per
        // `#[serde(rename_all = "camelCase")]` on `Profile`.
        (status = 200, description = "Profile body (see @spiritstream/types/Profile).", body = serde_json::Value),
        (status = 401, description = "Password required or incorrect.", body = ApiErrorBody),
        (status = 404, description = "Profile not found.", body = ApiErrorBody),
        (status = 500, description = "Internal server error.", body = ApiErrorBody),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_profile_show(
    State(state): State<AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    axum::extract::Query(q): axum::extract::Query<ProfileShowQuery>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    let profile = state
        .profile_manager
        .load_with_key_decryption(&name, q.password.as_deref())
        .await?;
    Ok(Json(serde_json::to_value(profile)?))
}

/// `PUT /profiles/{name}` — create or update a profile. The request body
/// contains the full profile + optional encryption password.
///
/// Server-side validation enforced (see `ProfileManager::save_with_key_encryption`):
/// - profile name charset/length
/// - RTMP input port-conflict with other profiles
/// - URL normalization via `PlatformRegistry::normalize_url`
#[utoipa::path(
    put,
    path = "/profiles/{name}",
    tag = "profiles",
    params(
        ("name" = String, Path, description = "Profile name (must match body's name)"),
    ),
    request_body = ProfileSaveRequest,
    responses(
        (status = 200, description = "Profile saved.", body = ProfileSaveResponse),
        (status = 400, description = "Validation failed.", body = ApiErrorBody),
        (status = 409, description = "Port conflict with another profile.", body = ApiErrorBody),
        (status = 500, description = "Internal server error.", body = ApiErrorBody),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_profile_save(
    State(state): State<AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    axum::Json(req): axum::Json<ProfileSaveRequest>,
) -> Result<Json<ProfileSaveResponse>, crate::ApiError> {
    let profile: spiritstream_core::models::Profile = serde_json::from_value(req.profile)?;
    if profile.name != name {
        return Err(crate::ApiError(
            spiritstream_core::CoreError::ValidationFailed {
                reasons: vec![spiritstream_core::errors::ValidationIssue {
                    code: "name_mismatch".into(),
                    message: "URL path and body name disagree.".into(),
                    path: Some("/name".into()),
                }],
            },
        ));
    }
    state
        .profile_manager
        .save_with_key_encryption(&profile, req.password.as_deref())
        .await?;
    Ok(Json(ProfileSaveResponse { name, saved: true }))
}

/// `DELETE /profiles/{name}` — remove a profile (encrypted or plaintext).
#[utoipa::path(
    delete,
    path = "/profiles/{name}",
    tag = "profiles",
    params(
        ("name" = String, Path, description = "Profile name"),
    ),
    responses(
        (status = 200, description = "Profile deleted.", body = ProfileDeleteResponse),
        (status = 404, description = "Profile not found.", body = ApiErrorBody),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_profile_delete(
    State(state): State<AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<Json<ProfileDeleteResponse>, crate::ApiError> {
    // Security guard: encrypted profiles cannot be deleted unless the
    // operator has unlocked them in this session via
    // `POST /profiles/:name/unlock` (or `/decrypt`). Without this
    // check the frontend's `unlockedProfiles.has(name)` short-circuit
    // was the only barrier against password-less deletion — a UI
    // bypass would silently destroy an encrypted profile.
    if state.profile_manager.is_encrypted(&name) {
        let unlocked = state.unlocked_profiles.lock().await;
        if !unlocked.contains(&name) {
            return Err(crate::ApiError::from(
                spiritstream_core::CoreError::PasswordRequired { name: name.clone() },
            ));
        }
    }
    state.profile_manager.delete(&name).await?;
    // Clear the session unlock for the deleted profile so a freshly
    // recreated namesake isn't accidentally treated as still-unlocked.
    state.unlocked_profiles.lock().await.remove(&name);
    Ok(Json(ProfileDeleteResponse {
        name,
        deleted: true,
    }))
}

/// `GET /profiles/{name}/encrypted` — quick check whether a profile is stored encrypted on disk.
#[utoipa::path(
    get,
    path = "/profiles/{name}/encrypted",
    tag = "profiles",
    params(
        ("name" = String, Path, description = "Profile name"),
    ),
    responses(
        (status = 200, description = "Encryption state.", body = ProfileIsEncryptedResponse),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_profile_is_encrypted(
    State(state): State<AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Json<ProfileIsEncryptedResponse> {
    let encrypted = state.profile_manager.is_encrypted(&name);
    Json(ProfileIsEncryptedResponse { name, encrypted })
}

// Profile activation, decrypt, lock.
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProfileActivateRequest {
    /// Password for encrypted profiles. Plaintext profiles ignore this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUnlockRequest {
    /// Required password — decryption fails on mismatch with 401.
    pub password: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUnlockResponse {
    pub name: String,
    /// Whether this profile is now in the session's unlocked set.
    pub unlocked: bool,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProfileLockResponse {
    pub name: String,
    pub locked: bool,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProfileLockedListResponse {
    /// Names of currently-unlocked encrypted profiles for this session.
    pub unlocked: Vec<String>,
}

/// `POST /profiles/{name}/activate` — load the profile, set it as the active
/// session profile, propagate to chat/OBS handlers, emit the consolidated
/// `profile_activated` event. The frontend used to do this cascade in
/// `profileStore.applyProfileSettings`; now it just listens for the event.
#[utoipa::path(
    post,
    path = "/profiles/{name}/activate",
    tag = "profiles",
    params(("name" = String, Path, description = "Profile name")),
    request_body = ProfileActivateRequest,
    responses(
        // Same Profile shape as GET /profiles/{name} — typed by ts-rs at
        // `@spiritstream/types/Profile`; see that handler for why utoipa
        // documents this as a free-form object.
        (status = 200, description = "Profile activated; body matches @spiritstream/types/Profile.", body = serde_json::Value),
        (status = 401, description = "Password required / incorrect.", body = ApiErrorBody),
        (status = 404, description = "Profile not found.", body = ApiErrorBody),
        (status = 409, description = "Activation precondition not met (e.g. no active profile resolvable).", body = ApiErrorBody),
        (status = 500, description = "Internal error during chat/OBS propagation.", body = ApiErrorBody),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_profile_activate(
    State(state): State<AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    axum::Json(req): axum::Json<ProfileActivateRequest>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    // Single call into the orchestrator. The service composes profile
    // load + OAuth refresh + chat/OBS propagation + `profile_activated`
    // bus emission. The transport handles only its own session state
    // (`set_active_profile`) and the UX-shaped follow-up event for
    // refresh failures.
    let outcome = state
        .profile_activation
        .activate(&name, req.password.as_deref())
        .await?;

    crate::set_active_profile(&state, &outcome.profile).await;

    for provider in &outcome.oauth_refresh_failed {
        state.event_bus.emit(
            "oauth_token_expired",
            serde_json::json!({ "provider": provider }),
        );
    }

    Ok(Json(serde_json::to_value(&outcome.profile)?))
}

/// `POST /profiles/{name}/unlock` — validate the password and add the
/// profile to the server-side session unlock set, replacing
/// `Profiles.tsx`'s frontend `unlockedProfiles: Set<string>` state.
#[utoipa::path(
    post,
    path = "/profiles/{name}/unlock",
    tag = "profiles",
    params(("name" = String, Path, description = "Profile name")),
    request_body = ProfileUnlockRequest,
    responses(
        (status = 200, description = "Password verified; profile marked unlocked.", body = ProfileUnlockResponse),
        (status = 401, description = "Password incorrect / required.", body = ApiErrorBody),
        (status = 404, description = "Profile not found.", body = ApiErrorBody),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_profile_unlock(
    State(state): State<AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    axum::Json(req): axum::Json<ProfileUnlockRequest>,
) -> Result<Json<ProfileUnlockResponse>, crate::ApiError> {
    // Verify decryption succeeds.
    let _profile = state
        .profile_manager
        .load_with_key_decryption(&name, Some(&req.password))
        .await?;
    let mut unlocked = state.unlocked_profiles.lock().await;
    unlocked.insert(name.clone());
    Ok(Json(ProfileUnlockResponse {
        name,
        unlocked: true,
    }))
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDecryptRequest {
    /// Password protecting the on-disk encrypted profile.
    pub password: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDecryptResponse {
    pub name: String,
    /// `true` once the profile is re-saved unencrypted on disk.
    pub decrypted: bool,
}

/// `POST /profiles/{name}/decrypt` — atomic encryption-removal.
///
/// Replaces the frontend's two-round-trip flow in
/// `apps/web/src/stores/profileStore.ts` (load with password → save without
/// password). The server loads the profile with the supplied password and
/// re-saves it WITHOUT encryption in one operation.
///
/// On success the profile also gets added to the session unlock set so
/// subsequent reads don't re-prompt for a password before the profile list
/// refreshes.
#[utoipa::path(
    post,
    path = "/profiles/{name}/decrypt",
    tag = "profiles",
    params(("name" = String, Path, description = "Profile name")),
    request_body = ProfileDecryptRequest,
    responses(
        (status = 200, description = "Encryption removed.", body = ProfileDecryptResponse),
        (status = 401, description = "Password incorrect / required.", body = ApiErrorBody),
        (status = 404, description = "Profile not found.", body = ApiErrorBody),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_profile_decrypt(
    State(state): State<AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    axum::Json(req): axum::Json<ProfileDecryptRequest>,
) -> Result<Json<ProfileDecryptResponse>, crate::ApiError> {
    // 1. Decrypt and load.
    let profile = state
        .profile_manager
        .load_with_key_decryption(&name, Some(&req.password))
        .await?;
    // 2. Re-save with no password — atomically removes encryption from disk.
    state
        .profile_manager
        .save_with_key_encryption(&profile, None)
        .await?;
    // 3. The session unlock set is now meaningless for this profile — drop it.
    let mut unlocked = state.unlocked_profiles.lock().await;
    unlocked.remove(&name);
    drop(unlocked);

    state.event_bus.emit(
        "profile_changed",
        serde_json::json!({ "action": "saved", "name": name }),
    );

    Ok(Json(ProfileDecryptResponse {
        name,
        decrypted: true,
    }))
}

/// `POST /profiles/{name}/lock` — remove the profile from the session unlock
/// set. After this call, accessing the profile again requires the password.
#[utoipa::path(
    post,
    path = "/profiles/{name}/lock",
    tag = "profiles",
    params(("name" = String, Path, description = "Profile name")),
    responses(
        (status = 200, description = "Profile relocked.", body = ProfileLockResponse),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_profile_lock(
    State(state): State<AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Json<ProfileLockResponse> {
    let mut unlocked = state.unlocked_profiles.lock().await;
    unlocked.remove(&name);
    Json(ProfileLockResponse { name, locked: true })
}

/// `GET /profiles/locked` — list every encrypted profile currently unlocked
/// in the session. The frontend uses this to render lock/unlock icons.
#[utoipa::path(
    get,
    path = "/profiles/locked",
    tag = "profiles",
    responses(
        (status = 200, description = "Session unlock state.", body = ProfileLockedListResponse),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub async fn v1_profile_locked_list(
    State(state): State<AppState>,
) -> Json<ProfileLockedListResponse> {
    let unlocked = state.unlocked_profiles.lock().await;
    Json(ProfileLockedListResponse {
        unlocked: unlocked.iter().cloned().collect(),
    })
}


// --------------------------------------------------------------------------
// Profiles — remaining proxies.

/// Wire mirror of [`spiritstream_core::models::ProfileSummary`]. The core
/// `services` field is `Vec<Platform>`, where `Platform` is auto-generated
/// from `data/streaming-platforms.json` at build time — each variant uses
/// `#[serde(rename = "Twitch")]` etc. so it serialises as the display
/// string. The wire mirror types that as `Vec<String>`, preserving the
/// wire shape while keeping utoipa out of core's build script.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummaryWire {
    pub id: String,
    pub name: String,
    pub resolution: String,
    pub bitrate: u32,
    pub target_count: u32,
    pub services: Vec<String>,
    pub is_encrypted: bool,
}

impl From<spiritstream_core::models::ProfileSummary> for ProfileSummaryWire {
    fn from(s: spiritstream_core::models::ProfileSummary) -> Self {
        Self {
            id: s.id,
            name: s.name,
            resolution: s.resolution,
            bitrate: s.bitrate,
            target_count: s.target_count,
            services: s
                .services
                .into_iter()
                .map(|p| serde_json::to_value(&p)
                    .ok()
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_default())
                .collect(),
            is_encrypted: s.is_encrypted,
        }
    }
}

/// Empty 200 ack body for handlers whose success payload is just
/// acknowledgement (validate / set order). Serialises as `{}`.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct ProfileAckResponse {}

/// `{indices: {name → order}}` envelope used by the `/profiles/order`
/// + `/profiles/order/ensure` endpoints. Wraps the raw map so OpenAPI
/// gets a named schema instead of an inline `additionalProperties`
/// object.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct ProfileOrderMapResponse {
    pub indices: std::collections::HashMap<String, u32>,
}

#[utoipa::path(get, path = "/profiles/summaries", tag = "profiles",
    responses(
        (status = 200, body = [ProfileSummaryWire], description = "Per-profile summary cards."),
        (status = 500, body = ApiErrorBody, description = "Internal error enumerating profiles."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_profile_summaries_proxy(
    State(state): State<AppState>,
) -> Result<Json<Vec<ProfileSummaryWire>>, crate::ApiError> {
    let summaries = state.profile_manager.get_all_summaries().await?;
    Ok(Json(summaries.into_iter().map(Into::into).collect()))
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProfileValidateInputRequest {
    pub profile_id: String,
    pub input: serde_json::Value,
}

#[utoipa::path(post, path = "/profiles/validate-input", tag = "profiles",
    request_body = ProfileValidateInputRequest,
    responses(
        (status = 200, body = ProfileAckResponse, description = "Input validates against other profiles."),
        (status = 400, body = ApiErrorBody, description = "Malformed RtmpInput payload."),
        (status = 409, body = ApiErrorBody, description = "Port conflict with another profile."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_profile_validate_input_proxy(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<ProfileValidateInputRequest>,
) -> Result<Json<ProfileAckResponse>, crate::ApiError> {
    let input: spiritstream_core::models::RtmpInput = serde_json::from_value(req.input)?;
    state
        .profile_manager
        .validate_input_conflict(&req.profile_id, &input)
        .await?;
    Ok(Json(ProfileAckResponse {}))
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProfileOrderSetRequest {
    pub ordered_names: Vec<String>,
}

#[utoipa::path(get, path = "/profiles/order", tag = "profiles",
    responses(
        (status = 200, body = ProfileOrderMapResponse, description = "Order index map."),
        (status = 500, body = ApiErrorBody, description = "Internal error reading order file."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_profile_order_get_proxy(
    State(state): State<AppState>,
) -> Result<Json<ProfileOrderMapResponse>, crate::ApiError> {
    let map = state.profile_manager.read_order_index_map()?;
    Ok(Json(ProfileOrderMapResponse {
        indices: map.into_iter().map(|(k, v)| (k, v as u32)).collect(),
    }))
}

#[utoipa::path(patch, path = "/profiles/order", tag = "profiles",
    request_body = ProfileOrderSetRequest,
    responses(
        (status = 200, body = ProfileAckResponse, description = "Order index map written."),
        (status = 404, body = ApiErrorBody, description = "One of the submitted profile names doesn't exist."),
        (status = 500, body = ApiErrorBody, description = "Internal error writing order file."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_profile_order_set_proxy(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<ProfileOrderSetRequest>,
) -> Result<Json<ProfileAckResponse>, crate::ApiError> {
    let mut map = state.profile_manager.read_order_index_map()?;
    let existing = state.profile_manager.get_all_names().await?;
    let mut idx = 0;
    for name in req.ordered_names {
        if !existing.contains(&name) {
            return Err(spiritstream_core::CoreError::ProfileNotFound { name }.into());
        }
        idx += 10;
        map.insert(name, idx);
    }
    state.profile_manager.write_order_index_map(&map)?;
    Ok(Json(ProfileAckResponse {}))
}

#[utoipa::path(post, path = "/profiles/order/ensure", tag = "profiles",
    responses(
        (status = 200, body = ProfileOrderMapResponse, description = "Order indexes ensured."),
        (status = 500, body = ApiErrorBody, description = "Internal error reading/writing order file."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_profile_order_ensure_proxy(
    State(state): State<AppState>,
) -> Result<Json<ProfileOrderMapResponse>, crate::ApiError> {
    let map = state.profile_manager.ensure_order_indexes().await?;
    Ok(Json(ProfileOrderMapResponse {
        indices: map.into_iter().map(|(k, v)| (k, v as u32)).collect(),
    }))
}
