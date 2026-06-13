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

use spiritstream_core::services::EventSink;

use crate::AppState;
// Imported by SHORT name so utoipa's response-`body` $ref matches the
// schema's registered name (`ChatMessageWire`, not `crate.v1.…`).
use crate::v1::ChatMessageWire;

// K5: wire-mirror types live in `v1/chat/wire.rs` and the log-status /
// export / search handlers in `v1/chat/log.rs` so this orchestrator
// stays under the 600 LOC ceiling. Re-export with their original
// public paths so the OpenAPI doc + downstream consumers see no
// contract change.
#[path = "chat/log.rs"]
pub mod log_handlers;
#[path = "chat/wire.rs"]
pub mod wire;
pub use log_handlers::{
    v1_chat_export_log_proxy, v1_chat_log_status_proxy, v1_chat_search_session_proxy,
    ChatExportRequest, ChatSearchRequest,
};
// utoipa's `paths(...)` macro looks for a `__path_<handler>` companion
// struct at the same module path as the handler use. Re-export those
// so the OpenApi derive in `v1/mod.rs` resolves them too.
pub use log_handlers::{
    __path_v1_chat_export_log_proxy, __path_v1_chat_log_status_proxy,
    __path_v1_chat_search_session_proxy,
};
pub use wire::{
    ChatAckResponse, ChatConfigWire, ChatConnectedResponse, ChatConnectionStatusWire,
    ChatCredentialsWire, ChatLogStatusWire, ChatPlatformStatusWire, ChatPlatformWire,
    ChatSendResultWire, TwitchAuthWire, YouTubeAuthWire,
};

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

/// `GET /chat/messages/recent` — the in-memory recent-message ring
/// (oldest→newest), the server-side replay source the frontend fetches
/// on every (re)connect / page load to repopulate chat. Sensitive chat
/// stays server-side (OWASP: never browser storage) and the response is
/// `Cache-Control: no-store` so the webview can't cache it (OWASP ASVS
/// anti-caching for sensitive data).
#[utoipa::path(get, path = "/chat/messages/recent", tag = "chat",
    responses((status = 200, body = Vec<ChatMessageWire>)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_chat_recent_messages_proxy(
    State(state): State<AppState>,
) -> Result<axum::response::Response, crate::ApiError> {
    use axum::response::IntoResponse;
    let wire: Vec<ChatMessageWire> = state
        .chat_manager
        .recent_messages()
        .await
        .into_iter()
        .map(Into::into)
        .collect();
    Ok((
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        Json(wire),
    )
        .into_response())
}

#[utoipa::path(post, path = "/chat/connections", tag = "chat",
    request_body = ChatConfigWire,
    responses(
        (status = 200, body = ChatAckResponse),
        (status = 400, body = ApiErrorBody),
        (status = 403, body = ApiErrorBody, description = "Confirm-token required for Facebook connect (intent=enable_facebook_chat)"),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_chat_connect_proxy(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    axum::Json(wire): axum::Json<ChatConfigWire>,
) -> Result<Json<ChatAckResponse>, crate::ApiError> {
    use spiritstream_core::models::{ChatCredentials, ChatPlatform, TwitchAuth, YouTubeAuth};

    let mut config: spiritstream_core::models::ChatConfig = wire.into();

    // Facebook gate: identity-revealing connect path must be deliberate.
    // The client first calls `POST /api/v1/security/confirm-token { intent:
    // "enable_facebook_chat" }`, presents the warning, and only on user
    // ack passes the returned token here via X-Confirm-Token. One-shot,
    // 30s TTL — same pattern as rotate_machine_key / clear_data.
    if matches!(config.platform, ChatPlatform::Facebook) {
        crate::require_confirm_token(&state, &headers, "enable_facebook_chat")?;
    }

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
        // The frontend never knows client ids — the transport resolves
        // Trovo's from the OAuth config chain (in-app setup → env →
        // embedded) and the connector fails loud on a placeholder.
        ChatCredentials::Trovo {
            channel_id,
            oauth_token,
            ..
        } => ChatCredentials::Trovo {
            channel_id,
            client_id: Some(state.oauth_service.get_config().await.get_trovo_client_id()),
            oauth_token,
        },
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
    // Chat is decoupled from streaming: connecting/reconnecting a
    // platform no longer requires an active stream. This endpoint backs
    // the per-platform Connect button. `ChatManager::connect` clears the
    // platform's disconnect-intent on success, so an explicit Connect
    // here also re-arms auto-connect/reconnect for it.
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
            crate::connect_trovo_chat(
                &state.chat_manager,
                &chat_settings,
                &profile_settings,
                state.oauth_service.get_config().await.get_trovo_client_id(),
                &state.event_bus,
            )
            .await;
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
    /// Optional per-message target override. When `Some`, dispatch
    /// only to the listed platforms — this can NARROW the broadcast
    /// set but never widen it: `ChatManager::send_message` re-checks
    /// the profile's `*_send_enabled` flags (and each connector's
    /// `can_send()` gate) for every target. When `None`, the handler
    /// auto-builds the target set from those same flags — the
    /// "broadcast to all enabled" behaviour controlled by
    /// `chatSettings.sendAllEnabled` on the frontend.
    ///
    /// Platform identifiers match the wire form of `ChatPlatform`:
    /// `twitch | youtube | trovo | kick | facebook | tiktok`.
    /// Unknown values are silently dropped to keep additive enum
    /// changes non-breaking.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_platforms: Option<Vec<String>>,
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

    let chat_settings = state.chat_manager.profile_chat_settings().await;
    let mut targets: Vec<ChatPlatform> = Vec::new();
    // Per-message override path: respect explicit picks from the
    // ChatComposer when `sendAllEnabled=false`. Each entry is mapped
    // back to a `ChatPlatform` enum value; unknown strings drop.
    if let Some(explicit) = req.target_platforms.as_ref() {
        for p in explicit {
            let mapped = match p.as_str() {
                "twitch" => Some(ChatPlatform::Twitch),
                "youtube" => Some(ChatPlatform::YouTube),
                "trovo" => Some(ChatPlatform::Trovo),
                "kick" => Some(ChatPlatform::Kick),
                "facebook" => Some(ChatPlatform::Facebook),
                "tiktok" => Some(ChatPlatform::TikTok),
                _ => None,
            };
            if let Some(platform) = mapped {
                if !targets.contains(&platform) {
                    targets.push(platform);
                }
            }
        }
    } else {
        // Broadcast path: derive from the per-platform send-enable flags
        // (the `sendAllEnabled=true` behaviour).
        if chat_settings.twitch_send_enabled {
            targets.push(ChatPlatform::Twitch);
        }
        if chat_settings.youtube_send_enabled && !chat_settings.youtube_use_api_key {
            targets.push(ChatPlatform::YouTube);
        }
        if chat_settings.trovo_send_enabled {
            targets.push(ChatPlatform::Trovo);
        }
        if chat_settings.kick_send_enabled {
            targets.push(ChatPlatform::Kick);
        }
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

// --------------------------------------------------------------------------
