//! Profile validation helpers — name shape, settings bounds, constants.

use crate::errors::{CoreError, ValidationIssue};

/// Discord webhook cooldown upper bound, in seconds. Zero is allowed
/// ("no rate limiting between notifications"); beyond 24 hours the
/// user has effectively disabled the notification and a typed setting
/// would be misleading.
pub const DISCORD_COOLDOWN_SECONDS_MAX: u32 = 86_400;

/// Backend bind port lower bound. Port 0 is reserved and never valid
/// for binding.
pub const BACKEND_PORT_MIN: u16 = 1;

/// Validate profile name to prevent path traversal attacks.
///
/// Returns `CoreError::ValidationFailed` with stable issue codes so callers
/// (CLI, frontend) can localize the message or branch programmatically.
pub(super) fn validate_profile_name(name: &str) -> Result<(), CoreError> {
    let mut reasons: Vec<ValidationIssue> = Vec::new();
    if name.is_empty() {
        reasons.push(ValidationIssue {
            code: "profile_name_empty".into(),
            message: "Profile name cannot be empty.".into(),
            path: Some("/name".into()),
        });
    }
    if name.contains('/') || name.contains('\\') {
        reasons.push(ValidationIssue {
            code: "profile_name_path_separator".into(),
            message: "Profile name cannot contain path separators.".into(),
            path: Some("/name".into()),
        });
    }
    if name.contains("..") {
        reasons.push(ValidationIssue {
            code: "profile_name_path_traversal".into(),
            message: "Profile name cannot contain '..'.".into(),
            path: Some("/name".into()),
        });
    }
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == ' ')
    {
        reasons.push(ValidationIssue {
            code: "profile_name_charset".into(),
            message:
                "Profile name can only contain letters, numbers, spaces, underscores, and hyphens."
                    .into(),
            path: Some("/name".into()),
        });
    }
    if name.len() > 100 {
        reasons.push(ValidationIssue {
            code: "profile_name_too_long".into(),
            message: "Profile name is too long (max 100 characters).".into(),
            path: Some("/name".into()),
        });
    }
    if reasons.is_empty() {
        Ok(())
    } else {
        Err(CoreError::ValidationFailed { reasons })
    }
}

impl super::ProfileManager {
    /// Enforce bounds on profile-scoped settings that now live in
    /// `ProfileSettings`. Out-of-range values produce a single
    /// `CoreError::ValidationFailed` carrying every offending field —
    /// callers get a complete list.
    pub(super) fn validate_profile_settings_bounds(
        settings: &crate::models::ProfileSettings,
    ) -> Result<(), CoreError> {
        let mut issues = Vec::new();
        if settings.backend.port < BACKEND_PORT_MIN {
            issues.push(ValidationIssue {
                code: "backend_port_out_of_range".into(),
                message: format!(
                    "backend.port must be in [{BACKEND_PORT_MIN}, 65535], got {}",
                    settings.backend.port
                ),
                path: Some("/settings/backend/port".into()),
            });
        }
        if settings.discord.cooldown_seconds > DISCORD_COOLDOWN_SECONDS_MAX {
            issues.push(ValidationIssue {
                code: "discord_cooldown_seconds_out_of_range".into(),
                message: format!(
                    "discord.cooldown_seconds must be in [0, {DISCORD_COOLDOWN_SECONDS_MAX}], got {}",
                    settings.discord.cooldown_seconds
                ),
                path: Some("/settings/discord/cooldownSeconds".into()),
            });
        }
        if issues.is_empty() {
            Ok(())
        } else {
            Err(CoreError::ValidationFailed { reasons: issues })
        }
    }
}
