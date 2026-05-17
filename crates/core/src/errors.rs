//! Structured error enum for SpiritStream core services.
//!
//! Every public service method returns `Result<T, CoreError>`. Transport
//! adapters (HTTP, CLI) are responsible for mapping variants to their
//! protocol-specific failure representations (HTTP status, exit code, etc.) —
//! that mapping never leaks into core. `From<std::io::Error>` and
//! `From<serde_json::Error>` impls let helpers use the `?` operator without
//! ad-hoc String adapters.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

/// One validation issue surfaced by a core service.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct ValidationIssue {
    /// Stable, machine-readable code (e.g. `"bitrate_out_of_range"`).
    pub code: String,
    /// Human-readable summary. Localization happens at the transport boundary.
    pub message: String,
    /// Optional pointer into the offending document (JSON-pointer-ish syntax).
    pub path: Option<String>,
}

/// Top-level error type returned by core services.
///
/// Variants are tagged for stable JSON serialization, so transports can
/// surface them directly without re-shaping the payload.
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
#[serde(tag = "kind", content = "details", rename_all = "snake_case")]
pub enum CoreError {
    #[error("profile not found: {name}")]
    ProfileNotFound { name: String },

    #[error("password required for profile: {name}")]
    PasswordRequired { name: String },

    #[error("password incorrect")]
    PasswordIncorrect,

    /// Password supplied to encrypt a profile is below the minimum length
    /// (see `crate::services::encryption::PROFILE_PASSWORD_MIN_LENGTH`).
    /// Distinct from `PasswordIncorrect` so the audit log can flag a
    /// weak-password rejection separately from brute-force attempts
    /// against an existing profile.
    #[error("password too short: minimum {min_length} characters")]
    PasswordTooShort { min_length: u32 },

    #[error("invalid stream config")]
    InvalidStreamConfig { reasons: Vec<ValidationIssue> },

    #[error("validation failed")]
    ValidationFailed { reasons: Vec<ValidationIssue> },

    #[error("profile already exists: {name}")]
    ProfileAlreadyExists { name: String },

    #[error("encoder unavailable: {encoder}")]
    EncoderUnavailable { encoder: String },

    #[error("port conflict on {port} (held by {owner})")]
    PortConflict { port: u16, owner: String },

    #[error("path outside allowed root")]
    PathOutsideAllowedRoot { path: String },

    /// Generic 404 for non-profile resources (files, themes by path, etc.).
    /// Use `ProfileNotFound` for the profile-specific case so callers can
    /// branch on profile-vs-other.
    #[error("not found: {resource}")]
    NotFound { resource: String },

    #[error("ffmpeg not found")]
    FfmpegNotFound,

    #[error("network error")]
    NetworkError { detail: String },

    #[error("rate limited")]
    RateLimited { retry_after_secs: u32 },

    #[error("unauthorized")]
    Unauthorized,

    /// No profile has been activated in this session — the caller is asking
    /// for state that lives behind `ProfileService::activate`. Distinct from
    /// `Unauthorized` so clients can prompt "choose a profile" instead of
    /// re-authenticating.
    #[error("no active profile")]
    NoActiveProfile,

    /// Chat platform connector has not been opened (or has dropped) before
    /// the caller asked it to send. Plan-cited variant: "new variants are
    /// added as the migration surfaces them (e.g., `ChatPlatformNotConnected
    /// { platform }` may emerge from `ChatService`)".
    #[error("chat platform not connected: {platform}")]
    ChatPlatformNotConnected { platform: String },

    /// The caller asked to send on a platform whose `*_send_enabled` flag is
    /// off in the active profile's chat settings. Separate from
    /// `ChatPlatformNotConnected` so the UI can suggest "enable sending" vs.
    /// "reconnect".
    #[error("chat sending disabled for platform: {platform}")]
    ChatSendingDisabled { platform: String },

    /// Outbound chat message exceeded the per-platform character cap (`limit`
    /// from `ChatPlatform::max_message_chars`). Validated server-side BEFORE
    /// the PII filter so the wire never sees the over-length text.
    #[error("chat message exceeds {limit}-char limit for {platform} ({actual} chars)")]
    ChatMessageLengthExceeded {
        platform: String,
        limit: usize,
        actual: usize,
    },

    /// Outbound chat message matched a phrase in the active
    /// profile's PII blocklist. `phrase_id` is the stable audit-log
    /// identifier; the actual matched text is never surfaced to the
    /// caller (would defeat the filter's purpose).
    #[error("chat message blocked by PII filter (phrase {phrase_id})")]
    ChatBlockedByPii { phrase_id: String },

    #[error("not implemented")]
    NotImplemented { feature: String },

    /// Catch-all for unexpected failures. The transport layer MUST NOT
    /// serialize the `context` field to clients — log server-side only.
    #[error("internal error")]
    Internal { context: String },
}

impl CoreError {
    /// Stable, machine-readable kind string. Mirrors the serde tag so external
    /// consumers (HTTP clients, CLI scripts, telemetry) can branch on this
    /// without parsing the human-readable message.
    pub fn kind(&self) -> &'static str {
        match self {
            CoreError::ProfileNotFound { .. } => "profile_not_found",
            CoreError::PasswordRequired { .. } => "password_required",
            CoreError::PasswordIncorrect => "password_incorrect",
            CoreError::PasswordTooShort { .. } => "password_too_short",
            CoreError::InvalidStreamConfig { .. } => "invalid_stream_config",
            CoreError::ValidationFailed { .. } => "validation_failed",
            CoreError::ProfileAlreadyExists { .. } => "profile_already_exists",
            CoreError::EncoderUnavailable { .. } => "encoder_unavailable",
            CoreError::PortConflict { .. } => "port_conflict",
            CoreError::PathOutsideAllowedRoot { .. } => "path_outside_allowed_root",
            CoreError::NotFound { .. } => "not_found",
            CoreError::FfmpegNotFound => "ffmpeg_not_found",
            CoreError::NetworkError { .. } => "network_error",
            CoreError::RateLimited { .. } => "rate_limited",
            CoreError::Unauthorized => "unauthorized",
            CoreError::NoActiveProfile => "no_active_profile",
            CoreError::ChatPlatformNotConnected { .. } => "chat_platform_not_connected",
            CoreError::ChatSendingDisabled { .. } => "chat_sending_disabled",
            CoreError::ChatMessageLengthExceeded { .. } => "chat_message_length_exceeded",
            CoreError::ChatBlockedByPii { .. } => "chat_blocked_by_pii",
            CoreError::NotImplemented { .. } => "not_implemented",
            CoreError::Internal { .. } => "internal",
        }
    }
}

impl From<std::io::Error> for CoreError {
    fn from(err: std::io::Error) -> Self {
        CoreError::Internal {
            context: format!("io: {err}"),
        }
    }
}

impl From<serde_json::Error> for CoreError {
    /// JSON deserialization failures from client-supplied bodies are
    /// **client errors** (malformed payload), not server bugs. Surface as
    /// `ValidationFailed` so the HTTP transport maps to 400 instead of 500
    /// and the client can show a field-level error message.
    fn from(err: serde_json::Error) -> Self {
        CoreError::ValidationFailed {
            reasons: vec![ValidationIssue {
                code: "malformed_json_body".into(),
                message: format!("serde_json: {err}"),
                path: None,
            }],
        }
    }
}
