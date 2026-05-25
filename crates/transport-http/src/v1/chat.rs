//! Chat handlers — `/api/v1/chat/*`.
//!
//! Connection lifecycle, send pipeline, status polling, log
//! status/export/search. The handlers here are thin shims over
//! `ChatManager` (in `spiritstream-core`); the OAuth-refresh-on-connect
//! and per-platform credential enrichment is the only real logic.
//!
//! Wire-mirror types (`*Wire`) below: utoipa is transport-only, so a
//! `ToSchema` derive on a core type would leak the transport. The
//! mirrors are thin newtypes with `From` impls — see the same pattern
//! in `v1/audit.rs::AuditChainStatusWire`.

use axum::{
    extract::{Path as AxumPath, State},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use spiritstream_core::models::{
    ChatConnectionStatus, ChatLogStatus, ChatPlatform, ChatPlatformStatus, ChatSendResult,
};
use spiritstream_core::services::EventSink;

use crate::AppState;

// ---------------------------------------------------------------------------
// Wire-mirror types for utoipa.
// ---------------------------------------------------------------------------

/// Mirror of [`ChatPlatform`] with `ToSchema`.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChatPlatformWire {
    Twitch,
    #[serde(rename = "tiktok")]
    TikTok,
    YouTube,
    Trovo,
    Stripchat,
    Kick,
    Facebook,
}

impl From<ChatPlatform> for ChatPlatformWire {
    fn from(value: ChatPlatform) -> Self {
        match value {
            ChatPlatform::Twitch => Self::Twitch,
            ChatPlatform::TikTok => Self::TikTok,
            ChatPlatform::YouTube => Self::YouTube,
            ChatPlatform::Trovo => Self::Trovo,
            ChatPlatform::Stripchat => Self::Stripchat,
            ChatPlatform::Kick => Self::Kick,
            ChatPlatform::Facebook => Self::Facebook,
        }
    }
}

/// Mirror of [`ChatConnectionStatus`] with `ToSchema`.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChatConnectionStatusWire {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

impl From<ChatConnectionStatus> for ChatConnectionStatusWire {
    fn from(value: ChatConnectionStatus) -> Self {
        match value {
            ChatConnectionStatus::Disconnected => Self::Disconnected,
            ChatConnectionStatus::Connecting => Self::Connecting,
            ChatConnectionStatus::Connected => Self::Connected,
            ChatConnectionStatus::Error => Self::Error,
        }
    }
}

/// Mirror of [`ChatPlatformStatus`] with `ToSchema`.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatPlatformStatusWire {
    pub platform: ChatPlatformWire,
    pub status: ChatConnectionStatusWire,
    pub message_count: u64,
    pub error: Option<String>,
}

impl From<ChatPlatformStatus> for ChatPlatformStatusWire {
    fn from(value: ChatPlatformStatus) -> Self {
        Self {
            platform: value.platform.into(),
            status: value.status.into(),
            message_count: value.message_count,
            error: value.error,
        }
    }
}

/// Mirror of [`ChatSendResult`] with `ToSchema`.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatSendResultWire {
    pub platform: ChatPlatformWire,
    pub success: bool,
    pub error: Option<String>,
    pub error_code: Option<String>,
}

impl From<ChatSendResult> for ChatSendResultWire {
    fn from(value: ChatSendResult) -> Self {
        Self {
            platform: value.platform.into(),
            success: value.success,
            error: value.error,
            error_code: value.error_code,
        }
    }
}

/// Mirror of [`ChatLogStatus`] with `ToSchema`. The core type already
/// has `serde` derives; the mirror exists purely to add `ToSchema`.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatLogStatusWire {
    pub active: bool,
    pub started_at: i64,
}

impl From<ChatLogStatus> for ChatLogStatusWire {
    fn from(value: ChatLogStatus) -> Self {
        Self {
            active: value.active,
            started_at: value.started_at,
        }
    }
}

/// Empty 200 OK response — used for handlers whose success payload is
/// just acknowledgement (`connect`, `disconnect`, `retry`, `export`).
/// Serialises as `{}`.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct ChatAckResponse {}

/// `GET /chat/connected` response.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatConnectedResponse {
    pub connected: bool,
}

// Chat.

#[utoipa::path(get, path = "/chat/connections", tag = "chat",
    responses((status = 200, body = Vec<ChatPlatformStatusWire>)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_chat_status_proxy(
    State(state): State<AppState>,
) -> Result<Json<Vec<ChatPlatformStatusWire>>, crate::ApiError> {
    let status = state.chat_manager.get_status().await;
    Ok(Json(status.into_iter().map(Into::into).collect()))
}

#[utoipa::path(post, path = "/chat/connections", tag = "chat",
    request_body = serde_json::Value,
    responses((status = 200, body = ChatAckResponse), (status = 400, body = ApiErrorBody)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_chat_connect_proxy(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<serde_json::Value>,
) -> Result<Json<ChatAckResponse>, crate::ApiError> {
    use spiritstream_core::models::{ChatConfig, ChatCredentials, TwitchAuth, YouTubeAuth};

    let mut config: ChatConfig = serde_json::from_value(req.get("config").cloned().unwrap_or(req))
        .map_err(|e| spiritstream_core::CoreError::Internal {
            context: format!("invalid ChatConfig payload: {e}"),
        })?;

    let mut profile_settings = crate::get_active_profile_settings(&state)
        .await
        .ok_or(spiritstream_core::CoreError::NoActiveProfile)?;

    config.credentials = match config.credentials {
        ChatCredentials::Twitch { channel, auth } => {
            let enriched_auth = match auth {
                Some(TwitchAuth::AppOAuth {
                    access_token,
                    refresh_token,
                    expires_at,
                }) if access_token.is_empty() => {
                    if profile_settings.oauth.twitch.access_token.is_empty() {
                        return Err(spiritstream_core::CoreError::Unauthorized.into());
                    }
                    // Fail loud on refresh failure: a silent fallback to
                    // the stale access token here used to send the chat
                    // connector live with a dead token (stream connects,
                    // hits Twitch, drops, reconnects — operator only saw
                    // a `warn!` line). Return `Unauthorized` instead so
                    // the frontend's 401 handling re-prompts re-auth and
                    // emits an `oauth_refresh_failed` event for the UI.
                    let fresh = crate::ensure_fresh_oauth_token(
                        "twitch",
                        &profile_settings.oauth.twitch.access_token,
                        &profile_settings.oauth.twitch.refresh_token,
                        profile_settings.oauth.twitch.expires_at,
                        &state.oauth_service,
                    )
                    .await
                    .map_err(|e| {
                        log::warn!("Twitch token refresh failed: {e}");
                        state.event_bus.emit(
                            "oauth_refresh_failed",
                            serde_json::json!({ "provider": "twitch", "detail": e.to_string() }),
                        );
                        crate::ApiError::from(spiritstream_core::CoreError::Unauthorized)
                    })?;
                    if fresh.refreshed {
                        profile_settings.oauth.twitch.access_token = fresh.access_token.clone();
                        if let Some(rt) = fresh.refresh_token.clone() {
                            profile_settings.oauth.twitch.refresh_token = rt;
                        }
                        profile_settings.oauth.twitch.expires_at = fresh.expires_at;
                        if let Err(err) =
                            crate::persist_active_profile_settings(&state, profile_settings.clone())
                                .await
                        {
                            log::warn!("Failed to persist Twitch OAuth refresh: {err}");
                        }
                    }
                    Some(TwitchAuth::AppOAuth {
                        access_token: fresh.access_token,
                        refresh_token: if refresh_token.is_none() {
                            Some(profile_settings.oauth.twitch.refresh_token.clone())
                                .filter(|s| !s.is_empty())
                        } else {
                            refresh_token
                        },
                        expires_at: if expires_at.is_none()
                            && profile_settings.oauth.twitch.expires_at > 0
                        {
                            Some(profile_settings.oauth.twitch.expires_at)
                        } else {
                            expires_at
                        },
                    })
                }
                other => other,
            };
            ChatCredentials::Twitch {
                channel,
                auth: enriched_auth,
            }
        }
        ChatCredentials::YouTube { channel_id, auth } => {
            let enriched_auth = match auth {
                YouTubeAuth::AppOAuth {
                    access_token,
                    refresh_token,
                    expires_at,
                } if access_token.is_empty() => {
                    if profile_settings.oauth.youtube.access_token.is_empty() {
                        return Err(spiritstream_core::CoreError::Unauthorized.into());
                    }
                    // Same fail-loud rule as the Twitch path: a stale-token
                    // fallback here sent the YouTube live-chat connector
                    // out with a dead bearer.
                    let fresh = crate::ensure_fresh_oauth_token(
                        "youtube",
                        &profile_settings.oauth.youtube.access_token,
                        &profile_settings.oauth.youtube.refresh_token,
                        profile_settings.oauth.youtube.expires_at,
                        &state.oauth_service,
                    )
                    .await
                    .map_err(|e| {
                        log::warn!("YouTube token refresh failed: {e}");
                        state.event_bus.emit(
                            "oauth_refresh_failed",
                            serde_json::json!({ "provider": "youtube", "detail": e.to_string() }),
                        );
                        crate::ApiError::from(spiritstream_core::CoreError::Unauthorized)
                    })?;
                    if fresh.refreshed {
                        profile_settings.oauth.youtube.access_token = fresh.access_token.clone();
                        if let Some(rt) = fresh.refresh_token.clone() {
                            profile_settings.oauth.youtube.refresh_token = rt;
                        }
                        profile_settings.oauth.youtube.expires_at = fresh.expires_at;
                        if let Err(err) =
                            crate::persist_active_profile_settings(&state, profile_settings.clone())
                                .await
                        {
                            log::warn!("Failed to persist YouTube OAuth refresh: {err}");
                        }
                    }
                    YouTubeAuth::AppOAuth {
                        access_token: fresh.access_token,
                        refresh_token: if refresh_token.is_none() {
                            Some(profile_settings.oauth.youtube.refresh_token.clone())
                                .filter(|s| !s.is_empty())
                        } else {
                            refresh_token
                        },
                        expires_at: if expires_at.is_none()
                            && profile_settings.oauth.youtube.expires_at > 0
                        {
                            Some(profile_settings.oauth.youtube.expires_at)
                        } else {
                            expires_at
                        },
                    }
                }
                other => other,
            };
            ChatCredentials::YouTube {
                channel_id,
                auth: enriched_auth,
            }
        }
        other => other,
    };

    state.chat_manager.connect(config).await?;
    Ok(Json(ChatAckResponse {}))
}

#[utoipa::path(delete, path = "/chat/connections", tag = "chat",
    responses((status = 200, body = ChatAckResponse)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_chat_disconnect_all_proxy(
    State(state): State<AppState>,
) -> Result<Json<ChatAckResponse>, crate::ApiError> {
    state.chat_manager.disconnect_all("user_requested").await?;
    Ok(Json(ChatAckResponse {}))
}

#[utoipa::path(get, path = "/chat/connections/{platform}", tag = "chat",
    params(("platform" = String, Path, description = "Chat platform")),
    responses((status = 200, body = Option<ChatPlatformStatusWire>)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_chat_platform_status_proxy(
    State(state): State<AppState>,
    AxumPath(platform): AxumPath<String>,
) -> Result<Json<Option<ChatPlatformStatusWire>>, crate::ApiError> {
    let platform_enum: spiritstream_core::models::ChatPlatform =
        serde_json::from_value(serde_json::json!(platform))?;
    let status = state.chat_manager.get_platform_status(platform_enum).await;
    Ok(Json(status.map(Into::into)))
}

#[utoipa::path(delete, path = "/chat/connections/{platform}", tag = "chat",
    params(("platform" = String, Path, description = "Chat platform")),
    responses(
        (status = 200, description = "Disconnected."),
        (status = 400, body = ApiErrorBody, description = "Unknown chat platform string."),
        (status = 422, body = ApiErrorBody, description = "Platform is not connected."),
        (status = 500, body = ApiErrorBody, description = "Connector disconnect failed."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_chat_disconnect_proxy(
    State(state): State<AppState>,
    AxumPath(platform): AxumPath<String>,
) -> Result<Json<ChatAckResponse>, crate::ApiError> {
    let platform_enum: spiritstream_core::models::ChatPlatform =
        serde_json::from_value(serde_json::json!(platform))?;
    state.chat_manager.disconnect(platform_enum).await?;
    Ok(Json(ChatAckResponse {}))
}

#[utoipa::path(post, path = "/chat/connections/{platform}/retry", tag = "chat",
    params(("platform" = String, Path, description = "Chat platform")),
    responses(
        (status = 200, description = "Reconnect triggered.", body = ChatAckResponse),
        (status = 400, body = ApiErrorBody, description = "Validation: chat not configured / retry unsupported / no active stream."),
        (status = 409, body = ApiErrorBody, description = "No active profile."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_chat_retry_proxy(
    State(state): State<AppState>,
    AxumPath(platform): AxumPath<String>,
) -> Result<Json<ChatAckResponse>, crate::ApiError> {
    use spiritstream_core::models::ChatPlatform;
    let platform_enum: ChatPlatform = serde_json::from_value(serde_json::json!(platform))?;
    let chat_settings = state.chat_manager.profile_chat_settings().await;
    let profile_settings = crate::get_active_profile_settings(&state)
        .await
        .ok_or(spiritstream_core::CoreError::NoActiveProfile)?;
    if state.ffmpeg_handler.active_count() == 0 {
        return Err(spiritstream_core::CoreError::ValidationFailed {
            reasons: vec![spiritstream_core::errors::ValidationIssue {
                code: "no_active_stream".into(),
                message: "Cannot reconnect chat when no stream is active".into(),
                path: None,
            }],
        }
        .into());
    }
    match platform_enum {
        ChatPlatform::Twitch => {
            if chat_settings.twitch_channel.is_empty() {
                return Err(spiritstream_core::CoreError::ValidationFailed {
                    reasons: vec![spiritstream_core::errors::ValidationIssue {
                        code: "chat_not_configured".into(),
                        message: "Twitch chat is not configured".into(),
                        path: Some("/settings/chat/twitch".into()),
                    }],
                }
                .into());
            }
            crate::connect_twitch_chat(
                &state.chat_manager,
                &chat_settings,
                &profile_settings,
                &state.event_bus,
            )
            .await;
        }
        ChatPlatform::Trovo => {
            if chat_settings.trovo_channel_id.is_empty() {
                return Err(spiritstream_core::CoreError::ValidationFailed {
                    reasons: vec![spiritstream_core::errors::ValidationIssue {
                        code: "chat_not_configured".into(),
                        message: "Trovo chat is not configured".into(),
                        path: Some("/settings/chat/trovo".into()),
                    }],
                }
                .into());
            }
            crate::connect_trovo_chat(&state.chat_manager, &chat_settings, &state.event_bus).await;
        }
        ChatPlatform::YouTube => {
            let has_oauth = !chat_settings.youtube_use_api_key
                && !profile_settings.oauth.youtube.access_token.is_empty();
            let has_api_key =
                chat_settings.youtube_use_api_key && !chat_settings.youtube_api_key.is_empty();
            if chat_settings.youtube_channel_id.is_empty() || (!has_oauth && !has_api_key) {
                return Err(spiritstream_core::CoreError::ValidationFailed {
                    reasons: vec![spiritstream_core::errors::ValidationIssue {
                        code: "chat_not_configured".into(),
                        message: "YouTube chat is not configured".into(),
                        path: Some("/settings/chat/youtube".into()),
                    }],
                }
                .into());
            }
            tokio::spawn(crate::connect_youtube_chat_with_retry(state.clone()));
        }
        _ => {
            return Err(spiritstream_core::CoreError::ValidationFailed {
                reasons: vec![spiritstream_core::errors::ValidationIssue {
                    code: "chat_retry_unsupported".into(),
                    message: "Retry not supported for this platform".into(),
                    path: Some("/platform".into()),
                }],
            }
            .into())
        }
    }
    Ok(Json(ChatAckResponse {}))
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatSendRequest {
    pub message: String,
}

#[utoipa::path(post, path = "/chat/messages", tag = "chat",
    request_body = ChatSendRequest,
    responses(
        (status = 200, body = Vec<ChatSendResultWire>, description = "Per-platform send results."),
        (status = 400, body = ApiErrorBody, description = "Empty message or no platforms enabled."),
        (status = 500, body = ApiErrorBody, description = "Internal error.")
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_chat_send_proxy(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<ChatSendRequest>,
) -> Result<Json<Vec<ChatSendResultWire>>, crate::ApiError> {
    use spiritstream_core::models::{ChatMessage, ChatPlatform, ChatSendResult};

    let trimmed = req.message.trim().to_string();
    if trimmed.is_empty() {
        return Err(spiritstream_core::CoreError::ValidationFailed {
            reasons: vec![spiritstream_core::errors::ValidationIssue {
                code: "chat_message_empty".into(),
                message: "Message cannot be empty".into(),
                path: Some("/message".into()),
            }],
        }
        .into());
    }

    let mut targets = Vec::new();
    let chat_settings = state.chat_manager.profile_chat_settings().await;
    if chat_settings.twitch_send_enabled {
        targets.push(ChatPlatform::Twitch);
    }
    if chat_settings.youtube_send_enabled && !chat_settings.youtube_use_api_key {
        targets.push(ChatPlatform::YouTube);
    }
    if chat_settings.trovo_send_enabled {
        targets.push(ChatPlatform::Trovo);
    }
    if chat_settings.stripchat_send_enabled {
        targets.push(ChatPlatform::Stripchat);
    }
    if targets.is_empty() {
        return Err(spiritstream_core::CoreError::ValidationFailed {
            reasons: vec![spiritstream_core::errors::ValidationIssue {
                code: "no_chat_platforms_enabled".into(),
                message: "No chat platforms are enabled for sending".into(),
                path: Some("/settings/chat".into()),
            }],
        }
        .into());
    }

    // PII filter, char-limit, dispatch, and audit-on-success now all
    // live inside `ChatManager::send_message`. Transport stays thin:
    // pull the cached PII snapshot, hand it through, surface results.
    let pii_policy = state.active_profile_pii.lock().await.clone();

    let results = state
        .chat_manager
        .send_message(trimmed.clone(), &targets, pii_policy, &state.safety)
        .await;

    // Atomic PII failure surfaces as `ChatBlockedByPii` on every result.
    // Translate that into the typed transport error before per-platform
    // shaping (kept here so the HTTP contract doesn't change — single
    // error response, not an array of failures).
    if let Some((_, Err(spiritstream_core::CoreError::ChatBlockedByPii { phrase_id }))) =
        results.first()
    {
        if results.iter().all(|(_, r)| {
            matches!(
                r,
                Err(spiritstream_core::CoreError::ChatBlockedByPii { .. })
            )
        }) {
            return Err(spiritstream_core::CoreError::ChatBlockedByPii {
                phrase_id: phrase_id.clone(),
            }
            .into());
        }
    }
    let mut send_results: Vec<ChatSendResult> = Vec::new();
    let mut successes: Vec<ChatPlatform> = Vec::new();
    for (platform, result) in results {
        match result {
            Ok(()) => {
                successes.push(platform);
                send_results.push(ChatSendResult {
                    platform,
                    success: true,
                    error: None,
                    error_code: None,
                });
            }
            Err(err) => {
                send_results.push(ChatSendResult {
                    platform,
                    success: false,
                    error: Some(err.to_string()),
                    error_code: Some(err.kind().to_string()),
                });
            }
        }
    }
    if !successes.is_empty() {
        let outbound = ChatMessage::new_outbound(successes, "You".to_string(), trimmed);
        state.chat_manager.log_message(outbound.clone());
        if let Ok(payload) = serde_json::to_value(&outbound) {
            use spiritstream_core::services::EventSink;
            state.event_bus.emit("chat_message", payload);
        }
    }
    Ok(Json(send_results.into_iter().map(Into::into).collect()))
}

#[utoipa::path(get, path = "/chat/connected", tag = "chat",
    responses((status = 200, body = ChatConnectedResponse)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_chat_is_connected_proxy(
    State(state): State<AppState>,
) -> Result<Json<ChatConnectedResponse>, crate::ApiError> {
    let connected = state.chat_manager.is_any_connected().await;
    Ok(Json(ChatConnectedResponse { connected }))
}

#[utoipa::path(get, path = "/chat/log", tag = "chat",
    responses((status = 200, body = ChatLogStatusWire)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_chat_log_status_proxy(
    State(state): State<AppState>,
) -> Result<Json<ChatLogStatusWire>, crate::ApiError> {
    let start_ms = state.chat_manager.log_session_start_ms();
    Ok(Json(ChatLogStatusWire {
        active: start_ms.is_some(),
        started_at: start_ms.unwrap_or(0),
    }))
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatExportRequest {
    pub path: String,
}

#[utoipa::path(post, path = "/chat/log/export", tag = "chat",
    request_body = ChatExportRequest,
    responses(
        (status = 200, description = "Chat log exported.", body = ChatAckResponse),
        (status = 400, body = ApiErrorBody, description = "No active chat session."),
        (status = 403, body = ApiErrorBody, description = "Export path outside allowed root."),
        (status = 500, body = ApiErrorBody, description = "Internal error writing export."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_chat_export_log_proxy(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<ChatExportRequest>,
) -> Result<Json<ChatAckResponse>, crate::ApiError> {
    use chrono::{Local, TimeZone};
    use spiritstream_core::models::ChatMessage;
    use std::fs::File;
    use std::io::{BufRead, BufReader, BufWriter, Write};

    // Validate the user-supplied export path stays inside the
    // data dir or home dir before opening — path_validator catches `..`
    // traversal + symlink escapes.
    let export_path = std::path::PathBuf::from(&req.path);
    let mut allowed_dirs: Vec<&std::path::Path> = vec![state.app_data_dir.as_path()];
    if let Some(ref home) = state.home_dir {
        allowed_dirs.push(home.as_path());
    }
    spiritstream_core::services::validate_path_within_any(&export_path, &allowed_dirs)?;

    let start_ms = state.chat_manager.log_session_start_ms().ok_or_else(|| {
        spiritstream_core::CoreError::ValidationFailed {
            reasons: vec![spiritstream_core::errors::ValidationIssue {
                code: "no_active_chat_session".into(),
                message: "No active chat session to export".into(),
                path: None,
            }],
        }
    })?;
    state.chat_manager.flush_chat_logs().await?;
    let end_ms = Local::now().timestamp_millis();
    let start_dt = Local
        .timestamp_millis_opt(start_ms)
        .single()
        .unwrap_or_else(Local::now);
    let end_dt = Local
        .timestamp_millis_opt(end_ms)
        .single()
        .unwrap_or_else(Local::now);
    let hour_keys = crate::build_hour_keys(start_dt, end_dt);
    let mut writer = BufWriter::new(File::create(&req.path).map_err(|e| {
        spiritstream_core::CoreError::Internal {
            context: format!("Failed to create export file: {e}"),
        }
    })?);
    for key in hour_keys {
        let src_path = state.log_dir.join(format!("chatlog_{}.jsonl", key));
        if !src_path.exists() {
            continue;
        }
        let file = File::open(&src_path).map_err(|e| spiritstream_core::CoreError::Internal {
            context: format!("Failed to read chat log {}: {e}", src_path.display()),
        })?;
        let reader = BufReader::new(file);
        for line in reader.lines() {
            let line = line.map_err(|e| spiritstream_core::CoreError::Internal {
                context: format!("Failed to read chat log: {e}"),
            })?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(message) = serde_json::from_str::<ChatMessage>(&line) {
                if message.timestamp >= start_ms && message.timestamp <= end_ms {
                    writer.write_all(line.as_bytes()).map_err(|e| {
                        spiritstream_core::CoreError::Internal {
                            context: format!("Failed to write export file: {e}"),
                        }
                    })?;
                    writer.write_all(b"\n").map_err(|e| {
                        spiritstream_core::CoreError::Internal {
                            context: format!("Failed to write export file: {e}"),
                        }
                    })?;
                }
            }
        }
    }
    writer
        .flush()
        .map_err(|e| spiritstream_core::CoreError::Internal {
            context: format!("Failed to finalize export file: {e}"),
        })?;
    Ok(Json(ChatAckResponse {}))
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatSearchRequest {
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

// Search response is `Vec<ChatMessage>`. `ChatMessage` lives in
// `spiritstream-core` and has no `ToSchema` (utoipa is transport-only).
// Mirroring it would mean duplicating ~30 nested fragment/payload variants
// that are actively in flux per the in-flight chat-features branch; the
// schema lands once that work settles. The wire format is still typed by
// serde (the `ChatMessage` Serialize impl); only the OpenAPI surface is
// loose for now, matching what `@hey-api/openapi-ts` already emits as
// `unknown[]` for the api-client.
#[utoipa::path(post, path = "/chat/log/search", tag = "chat",
    request_body = ChatSearchRequest,
    responses((status = 200, body = Vec<serde_json::Value>,
        description = "Matching ChatMessage entries (typed schema deferred until in-flight ChatMessage shape stabilises).")),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_chat_search_session_proxy(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<ChatSearchRequest>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    use chrono::{Local, TimeZone};
    use spiritstream_core::models::ChatMessage;
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let limit = req.limit.unwrap_or(500);
    let start_ms = state.chat_manager.log_session_start_ms().ok_or_else(|| {
        spiritstream_core::CoreError::ValidationFailed {
            reasons: vec![spiritstream_core::errors::ValidationIssue {
                code: "no_active_chat_session".into(),
                message: "No active chat session to search".into(),
                path: None,
            }],
        }
    })?;
    let query = req.query.trim().to_lowercase();
    if query.is_empty() {
        return Ok(Json(serde_json::json!([])));
    }
    let end_ms = Local::now().timestamp_millis();
    let start_dt = Local
        .timestamp_millis_opt(start_ms)
        .single()
        .unwrap_or_else(Local::now);
    let end_dt = Local
        .timestamp_millis_opt(end_ms)
        .single()
        .unwrap_or_else(Local::now);
    let hour_keys = crate::build_hour_keys(start_dt, end_dt);
    let mut matches: Vec<ChatMessage> = Vec::new();
    for key in hour_keys {
        if matches.len() >= limit {
            break;
        }
        let src_path = state.log_dir.join(format!("chatlog_{}.jsonl", key));
        if !src_path.exists() {
            continue;
        }
        let file = File::open(&src_path).map_err(|e| spiritstream_core::CoreError::Internal {
            context: format!("Failed to read chat log {}: {e}", src_path.display()),
        })?;
        let reader = BufReader::new(file);
        for line in reader.lines() {
            if matches.len() >= limit {
                break;
            }
            let line = line.map_err(|e| spiritstream_core::CoreError::Internal {
                context: format!("Failed to read chat log: {e}"),
            })?;
            if line.trim().is_empty() {
                continue;
            }
            let message = match serde_json::from_str::<ChatMessage>(&line) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if message.timestamp < start_ms || message.timestamp > end_ms {
                continue;
            }
            let username = message.username.to_lowercase();
            let text = message.message.to_lowercase();
            if username.contains(&query) || text.contains(&query) {
                matches.push(message);
            }
        }
    }
    Ok(Json(serde_json::json!(matches)))
}

// --------------------------------------------------------------------------
