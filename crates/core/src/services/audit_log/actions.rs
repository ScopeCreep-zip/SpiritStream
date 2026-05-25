use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One discrete entry, including its position in and contribution to
/// the HMAC chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    /// Position in the append-only sequence. Starts at 1 for the
    /// first entry; strictly monotonic per audit-log file.
    #[serde(default)]
    pub seq: u64,
    /// Monotonic UTC timestamp for the event.
    pub timestamp: DateTime<Utc>,
    /// Structured event kind. Use one of the [`AuditAction`] variants
    /// to ensure the field name space stays bounded.
    pub action: AuditAction,
    /// Optional human-readable detail. **Must not** carry secret
    /// values (OAuth tokens, message bodies, passwords). Limit to
    /// short metadata — IDs, counts, platform names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Hex-encoded HMAC of the previous entry, or 64 zero hex chars
    /// for the first entry in the file.
    #[serde(default)]
    pub prev_hmac: String,
    /// Hex-encoded HMAC-SHA256 of this entry's canonical bytes.
    /// This is the chain's tamper-evidence anchor.
    #[serde(default)]
    pub hmac: String,
}

/// Enumerated set of audit event kinds. Adding a new variant is a
/// deliberate act — every recordable action passes through this enum.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuditAction {
    PanicTriggered {
        streams_stopped: usize,
        elapsed_ms: u64,
    },
    /// Outbound chat send was blocked because the message matched a
    /// phrase in the active profile's PII blocklist. One entry per
    /// send-call, not per-platform — the filter decision is
    /// platform-agnostic, so per-platform records would just inflate
    /// the chain. `platforms` is the full list of targets the send
    /// attempted; `phrase_id` is the HMAC-keyed stable ID of the
    /// matched phrase (**never** the matched text itself).
    ChatMessagePiiBlocked {
        platforms: Vec<String>,
        phrase_id: String,
    },
    ProfileSaved {
        name: String,
    },
    ProfileDeleted {
        name: String,
    },
    OauthRefresh {
        platform: String,
        success: bool,
    },
    OauthRefreshUnusualLocation {
        platform: String,
    },
    MachineKeyRotated {
        profiles_updated: usize,
        keys_reencrypted: usize,
    },
    AnonymousModeToggled {
        enabled: bool,
    },
    AppStarted,
    AppStopped,
    AuditLogTamperDetected {
        last_valid_sequence: u64,
    },
    /// A theme file on disk failed `ThemeManager::validate_theme()` —
    /// operator-visible signal for incomplete or malformed themes
    /// (e.g. accessibility regression where a high-contrast theme is
    /// missing required tokens). The WARN log alone is not user-visible
    /// and not searchable; the audit log makes regressions traceable.
    ThemeValidationFailed {
        file_name: String,
        reason: String,
    },
    /// The Tauri 2 self-updater rejected a downloaded
    /// release artifact's `.sig` against the embedded ed25519 pubkey,
    /// or the download itself failed in a way the user surface
    /// interprets as security-relevant. The frontend records this via
    /// `POST /api/v1/system/audit/app-update-failure` after a check /
    /// install error. Operators grep `app_update_signature_failed` in
    /// the chain to spot tampered-update attempts.
    AppUpdateSignatureFailed {
        detail: String,
    },
    /// One or more chat platforms accepted an outbound message from
    /// the broadcaster. `platforms` lists each successful destination;
    /// `char_count` is the grapheme-conservative `chars().count()` of
    /// the message body. Message text itself is **never** persisted —
    /// only the metadata needed to reconstruct "what did SpiritStream
    /// send, when, where" for the moderation audit trail.
    ChatMessageSent {
        platforms: Vec<String>,
        char_count: usize,
    },
    /// A chat platform connector successfully completed its `connect`
    /// handshake. `account_id` is the platform-side identifier when
    /// known (Twitch user_id, YouTube channel ID, …). Connectors that
    /// don't yet surface it pass `None`; Phase D wiring fills these in.
    ChatPlatformConnected {
        platform: String,
        account_id: Option<String>,
    },
    /// A chat platform connector was disconnected. `reason` carries a
    /// short discriminator (`"user_requested"`, `"connection_lost"`,
    /// `"panic_triggered"`, `"shutdown"`) — not free-form so operators
    /// can grep the chain by reason category.
    ChatPlatformDisconnected {
        platform: String,
        reason: String,
    },
}

/// Result of a chain-verification pass. Held in memory by the
/// transport layer and surfaced to the audit-log UI as a banner.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuditChainStatus {
    /// Every entry's HMAC verified.
    Ok { entries_verified: u64 },
    /// Verification failed at the named sequence. The red
    /// banner displays `lastValidSequence` so the user knows how much
    /// of the log they can still trust.
    Tampered {
        last_valid_sequence: u64,
        reason: String,
    },
    /// Log file does not exist yet (fresh install).
    Empty,
}
