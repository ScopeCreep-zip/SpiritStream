//! Audit log read endpoint — `/api/v1/audit/log`.
//!
//! Reads + paginates the HMAC-chained audit log produced by
//! `AuditLogService`, plus a tamper-status banner field that drives
//! the red banner in the audit log UI.

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::AppState;

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
