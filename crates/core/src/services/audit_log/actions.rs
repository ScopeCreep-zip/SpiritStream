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
        /// Q8: errors observed during the panic disconnect sequence.
        /// Pre-Q8 these were logged-and-forgotten; the panic audit row
        /// only said "panic ran" without telling the operator that, e.g.,
        /// the Twitch disconnect timed out — vital context for the
        /// post-incident "did my chat actually disconnect?" question.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        connector_errors: Vec<String>,
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
    /// OAuth refresh-frequency anomaly (rapid-burst heuristic). The
    /// serialized kind is pinned to the historical string: the HMAC
    /// chain re-serializes actions during verification, so changing
    /// the wire form would false-tamper every existing log. No
    /// geolocation is involved — the name predates the heuristic.
    #[serde(rename = "oauth_refresh_unusual_location")]
    OauthRefreshAnomaly {
        platform: String,
    },
    MachineKeyRotated {
        profiles_updated: usize,
        keys_reencrypted: usize,
    },
    /// A panic / stop killed FFmpeg processes recorded by another
    /// (possibly dead) SpiritStream process via the cross-process
    /// registry (`run/stream_processes.json`). `stale` counts records
    /// whose pid was gone or no longer FFmpeg (dropped, never killed).
    PanicKilledOrphans { killed: usize, stale: usize },
    /// Startup recovery found an interrupted key rotation and rolled
    /// back to the previous key (profiles restored from backup).
    KeyRotationRolledBack,
    /// Startup recovery found an interrupted key rotation whose old key
    /// was already shredded, and promoted the pending key — the
    /// rotation is now complete.
    KeyRotationRecovered,
    AnonymousModeToggled {
        enabled: bool,
    },
    AppStarted,
    AppStopped,
    AuditLogTamperDetected {
        last_valid_sequence: u64,
    },
    /// The legacy single-key chain verified intact and was archived to
    /// `audit.log.v1-archive`; this fresh chain uses per-day keys.
    /// `verify_archive` keeps the archived history checkable.
    ChainMigrated {
        archived_entries: u64,
    },
    /// The previous log file contained an unparseable line — a tamper
    /// or torn-write signal — and was preserved at
    /// `audit.log.quarantined-<ts>` while this fresh chain started.
    /// Startup must never brick on a corrupt log; the quarantine + this
    /// entry keep the event loud and investigable.
    ChainQuarantined {
        reason: String,
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
    /// A Discord go-live webhook send was attempted. H9 — webhook
    /// posts are user-visible side effects (announcement messages
    /// land in a Discord channel attached to a real audience) and
    /// belong in the audit trail. `success` distinguishes accepted
    /// posts from upstream rejections; `skipped_cooldown` records
    /// the cooldown-suppressed path so operators understand why a
    /// "go live" didn't reach Discord even when their settings
    /// looked enabled. Webhook URL itself is never recorded.
    DiscordWebhookSent {
        success: bool,
        skipped_cooldown: bool,
    },
    /// `POST /api/v1/security/sessions/revoke-all` ran and dropped
    /// every active session ID. `count` is how many sessions were
    /// in the active set at revoke time — a stolen-session incident
    /// shows up as a non-trivial number here. G2.
    SessionRevoked {
        count: usize,
    },
    /// A one-shot confirm token was issued for a destructive intent
    /// (`clear_data`, `rotate_machine_key`, `revoke_all_sessions`,
    /// `enable_facebook_chat`, …). The token itself is **never**
    /// recorded; only the intent string + the timestamp lets
    /// operators reconstruct "did the user ask for X around time T"
    /// when reviewing post-incident. Issue is paired with the
    /// downstream `MachineKeyRotated` / `SessionRevoked` / etc.
    /// emission via wall-clock proximity in the chain. G2.
    ConfirmTokenIssued {
        intent: String,
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
