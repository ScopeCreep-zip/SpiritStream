//! Chat log status / export / search handlers.
//!
//! K5 split: pulled out of `v1/chat.rs` so the orchestrator stays
//! under the 600 LOC ceiling. Each handler operates on the rotating
//! per-hour `chatlog_*.jsonl` files written by `ChatManager`.

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::AppState;

use super::{ChatAckResponse, ChatLogStatusWire};

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
    use std::fs::File;
    use std::io::{BufWriter, Write};

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
        let src_path = state.log_dir.join(format!("chatlog_{}.enc", key));
        if !src_path.exists() {
            continue;
        }
        // The on-disk history is encrypted at rest — decrypt the records
        // here. The user-chosen export file is a deliberate plaintext
        // export (their explicit act).
        for message in
            spiritstream_core::services::read_messages_from_file(&src_path, &state.app_data_dir)
        {
            if message.timestamp >= start_ms && message.timestamp <= end_ms {
                if let Ok(line) = serde_json::to_string(&message) {
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

// G5: search response is the fully-typed `Vec<ChatMessageWire>`. The
// wire mirror lives in `v1/chat_message_wire.rs` and is byte-identical
// to the ts-rs export at `@spiritstream/types/ChatMessage`. Replaces
// the prior `Vec<serde_json::Value>` placeholder.
#[utoipa::path(post, path = "/chat/log/search", tag = "chat",
    request_body = ChatSearchRequest,
    responses((status = 200, body = Vec<crate::v1::ChatMessageWire>,
        description = "Matching ChatMessage entries.")),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_chat_search_session_proxy(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<ChatSearchRequest>,
) -> Result<Json<Vec<crate::v1::ChatMessageWire>>, crate::ApiError> {
    use chrono::{Local, TimeZone};
    use spiritstream_core::models::ChatMessage;

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
        return Ok(Json(Vec::new()));
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
        let src_path = state.log_dir.join(format!("chatlog_{}.enc", key));
        if !src_path.exists() {
            continue;
        }
        // Decrypt the encrypted-at-rest history records for searching.
        for message in
            spiritstream_core::services::read_messages_from_file(&src_path, &state.app_data_dir)
        {
            if matches.len() >= limit {
                break;
            }
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
    Ok(Json(
        matches
            .into_iter()
            .map(Into::into)
            .collect::<Vec<crate::v1::ChatMessageWire>>(),
    ))
}
