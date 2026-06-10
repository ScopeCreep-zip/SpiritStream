//! Audit log read endpoint — `/api/v1/audit/log`.
//!
//! Reads + paginates the HMAC-chained audit log produced by
//! `AuditLogService`, plus a tamper-status banner field that drives
//! the red banner in the audit log UI.

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use spiritstream_core::services::AuditChainStatus;

use crate::AppState;

/// Wire-mirror of [`AuditChainStatus`] with `ToSchema` for OpenAPI.
/// utoipa is a transport-only dep — keeping `ToSchema` on a core type
/// would leak the transport into core. The variants and field shapes
/// must stay in lockstep; the `From<AuditChainStatus>` impl below is
/// the single conversion point so any drift surfaces at compile time.
///
/// Serialises as a discriminated union on `state` so the TS client
/// reads `chain.state === 'tampered'` directly:
///
/// ```json
/// {"state": "ok", "entriesVerified": 42}
/// {"state": "tampered", "lastValidSequence": 5, "reason": "hmac mismatch at seq 6"}
/// {"state": "empty"}
/// ```
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AuditChainStatusWire {
    /// Every entry's HMAC verified.
    Ok { entries_verified: u64 },
    /// Verification failed at the named sequence. The red banner
    /// displays `lastValidSequence` so the user knows how much of the
    /// log they can still trust.
    Tampered {
        last_valid_sequence: u64,
        reason: String,
    },
    /// Log file does not exist yet (fresh install).
    Empty,
}

impl From<AuditChainStatus> for AuditChainStatusWire {
    fn from(value: AuditChainStatus) -> Self {
        match value {
            AuditChainStatus::Ok { entries_verified } => {
                AuditChainStatusWire::Ok { entries_verified }
            }
            AuditChainStatus::Tampered {
                last_valid_sequence,
                reason,
            } => AuditChainStatusWire::Tampered {
                last_valid_sequence,
                reason,
            },
            AuditChainStatus::Empty => AuditChainStatusWire::Empty,
        }
    }
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
    pub chain: AuditChainStatusWire,
    /// Tail-anchor persistence state: `"ok"` or `"degraded"` (anchor
    /// writes to the secret store are failing, so truncation detection
    /// is impaired until it recovers). Loud by design.
    pub anchor_state: String,
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
    // Verify the HMAC chain before serving entries. We surface the
    // chain status — Ok / Tampered / Empty — to the client either way
    // so the UI can show the tamper banner even when the user has
    // filtered the page to an empty result.
    //
    // G3: an Err from `verify_chain` is an I/O / read failure (the
    // chain file couldn't even be opened), NOT a tamper. Pre-G3 we
    // conflated the two by force-bucketing every Err into a
    // synthetic `Tampered { last_valid_sequence: 0, reason: ... }`
    // payload — that masked permission/disk errors as security events.
    // Now we propagate the read failure as 500 so operators see it
    // for what it is.
    let chain_status = state.audit.verify_chain()?;
    let chain: AuditChainStatusWire = chain_status.into();

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
        anchor_state: state.audit.anchor_state().to_string(),
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
        OauthRefreshAnomaly { .. } => "oauth_refresh_unusual_location",
        MachineKeyRotated { .. } => "machine_key_rotated",
        PanicKilledOrphans { .. } => "panic_killed_orphans",
        KeyRotationRolledBack => "key_rotation_rolled_back",
        KeyRotationRecovered => "key_rotation_recovered",
        AnonymousModeToggled { .. } => "anonymous_mode_toggled",
        AppStarted => "app_started",
        AppStopped => "app_stopped",
        AuditLogTamperDetected { .. } => "audit_log_tamper_detected",
        ChainMigrated { .. } => "chain_migrated",
        ChainQuarantined { .. } => "chain_quarantined",
        ThemeValidationFailed { .. } => "theme_validation_failed",
        AppUpdateSignatureFailed { .. } => "app_update_signature_failed",
        ChatMessageSent { .. } => "chat_message_sent",
        ChatPlatformConnected { .. } => "chat_platform_connected",
        ChatPlatformDisconnected { .. } => "chat_platform_disconnected",
        DiscordWebhookSent { .. } => "discord_webhook_sent",
        SessionRevoked { .. } => "session_revoked",
        ConfirmTokenIssued { .. } => "confirm_token_issued",
    }
}
