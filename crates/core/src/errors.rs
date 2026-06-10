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

    /// Anonymous mode is enabled but the profile's pseudonymizer salt
    /// is empty or not valid hex. Pseudonymization fails loud rather
    /// than silently passing real usernames through — for the people
    /// this app serves, a chat log with plaintext usernames while the
    /// UI says "anonymous" is a doxxing vector, not a degraded mode.
    #[error("anonymous-mode salt is missing or invalid")]
    AnonymousSaltInvalid,

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
            CoreError::AnonymousSaltInvalid => "anonymous_salt_invalid",
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variant's `kind()` must equal its serde tag, and the
    /// strings must be unique — external consumers branch on these.
    #[test]
    fn kind_strings_are_exhaustive_and_unique() {
        let variants = vec![
            CoreError::ProfileNotFound { name: "p".into() },
            CoreError::PasswordRequired { name: "p".into() },
            CoreError::PasswordIncorrect,
            CoreError::PasswordTooShort { min_length: 12 },
            CoreError::InvalidStreamConfig { reasons: vec![] },
            CoreError::ValidationFailed { reasons: vec![] },
            CoreError::ProfileAlreadyExists { name: "p".into() },
            CoreError::EncoderUnavailable {
                encoder: "x".into(),
            },
            CoreError::PortConflict {
                port: 1935,
                owner: "p".into(),
            },
            CoreError::PathOutsideAllowedRoot { path: "/x".into() },
            CoreError::NotFound {
                resource: "r".into(),
            },
            CoreError::FfmpegNotFound,
            CoreError::NetworkError { detail: "d".into() },
            CoreError::RateLimited {
                retry_after_secs: 5,
            },
            CoreError::Unauthorized,
            CoreError::NoActiveProfile,
            CoreError::ChatPlatformNotConnected {
                platform: "twitch".into(),
            },
            CoreError::ChatSendingDisabled {
                platform: "twitch".into(),
            },
            CoreError::ChatMessageLengthExceeded {
                platform: "twitch".into(),
                limit: 500,
                actual: 600,
            },
            CoreError::ChatBlockedByPii {
                phrase_id: "ph".into(),
            },
            CoreError::NotImplemented {
                feature: "veilid".into(),
            },
            CoreError::Internal {
                context: "boom".into(),
            },
        ];

        let mut seen = std::collections::HashSet::new();
        for err in &variants {
            let kind = err.kind();
            assert!(!kind.is_empty(), "kind must be non-empty for {err:?}");
            assert!(seen.insert(kind), "duplicate kind string: {kind}");
        }
        assert_eq!(seen.len(), variants.len());
    }

    #[test]
    fn kind_matches_serde_tag() {
        let err = CoreError::PortConflict {
            port: 1935,
            owner: "live".into(),
        };
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["kind"], err.kind());
    }

    #[test]
    fn display_interpolates_fields() {
        let err = CoreError::PortConflict {
            port: 1935,
            owner: "live".into(),
        };
        assert_eq!(err.to_string(), "port conflict on 1935 (held by live)");
        assert_eq!(
            CoreError::ProfileNotFound {
                name: "alpha".into()
            }
            .to_string(),
            "profile not found: alpha"
        );
    }

    #[test]
    fn io_error_maps_to_internal() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err: CoreError = io.into();
        match err {
            CoreError::Internal { context } => assert!(context.contains("io:")),
            other => panic!("expected Internal, got {other:?}"),
        }
    }

    #[test]
    fn serde_json_error_maps_to_validation_failed() {
        let bad: Result<i32, _> = serde_json::from_str("not json");
        let err: CoreError = bad.unwrap_err().into();
        match err {
            CoreError::ValidationFailed { reasons } => {
                assert_eq!(reasons[0].code, "malformed_json_body");
            }
            other => panic!("expected ValidationFailed, got {other:?}"),
        }
    }
}
