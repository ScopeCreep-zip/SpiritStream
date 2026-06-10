//! Profile validation helpers — name shape, settings bounds, constants.

use crate::errors::{CoreError, ValidationIssue};

/// Discord webhook cooldown upper bound, in seconds. Zero is allowed
/// ("no rate limiting between notifications"); beyond 24 hours the
/// user has effectively disabled the notification and a typed setting
/// would be misleading.
pub const DISCORD_COOLDOWN_SECONDS_MAX: u32 = 86_400;

/// Backend bind port lower bound. Matches the env-path rule
/// (`SPIRITSTREAM_PORT` must be 1024..=65535): privileged ports need
/// root and port 0 is reserved — a profile-sourced port below 1024
/// used to pass validation here and then fail at bind time instead of
/// surfacing at save time.
pub const BACKEND_PORT_MIN: u16 = 1024;

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

/// Normalise the PII blocklist in place: trim whitespace, drop empty
/// entries, dedupe (first occurrence wins, order preserved). Runs in
/// the core save path so EVERY writer — safety wizard, settings panel,
/// CLI profile save — produces the same canonical list; the frontend
/// passes raw user input through verbatim.
pub(super) fn normalize_pii_blocklist(entries: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    let mut normalized = Vec::with_capacity(entries.len());
    for entry in entries.drain(..) {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            normalized.push(trimmed.to_string());
        }
    }
    *entries = normalized;
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

#[cfg(test)]
mod tests {
    use super::super::ProfileManager;
    use super::validate_profile_name;
    use crate::errors::CoreError;
    use crate::models::ProfileSettings;

    fn codes(err: CoreError) -> Vec<String> {
        match err {
            CoreError::ValidationFailed { reasons } => {
                reasons.into_iter().map(|r| r.code).collect()
            }
            other => panic!("expected ValidationFailed, got {other:?}"),
        }
    }

    #[test]
    fn ordinary_names_pass() {
        validate_profile_name("My Stream-1_alpha").expect("clean name accepted");
    }

    #[test]
    fn empty_name_is_rejected() {
        assert_eq!(
            codes(validate_profile_name("").unwrap_err()),
            vec!["profile_name_empty"]
        );
    }

    #[test]
    fn path_separators_and_traversal_are_rejected() {
        assert!(codes(validate_profile_name("a/b").unwrap_err())
            .contains(&"profile_name_path_separator".to_string()));
        assert!(codes(validate_profile_name("a\\b").unwrap_err())
            .contains(&"profile_name_path_separator".to_string()));
        assert!(codes(validate_profile_name("../etc").unwrap_err())
            .contains(&"profile_name_path_traversal".to_string()));
    }

    #[test]
    fn disallowed_characters_are_rejected() {
        assert!(codes(validate_profile_name("bad$name").unwrap_err())
            .contains(&"profile_name_charset".to_string()));
    }

    #[test]
    fn overlong_name_is_rejected() {
        let long = "a".repeat(101);
        assert!(codes(validate_profile_name(&long).unwrap_err())
            .contains(&"profile_name_too_long".to_string()));
    }

    #[test]
    fn default_settings_are_within_bounds() {
        ProfileManager::validate_profile_settings_bounds(&ProfileSettings::default())
            .expect("defaults are valid");
    }

    #[test]
    fn zero_backend_port_is_out_of_range() {
        let mut settings = ProfileSettings::default();
        settings.backend.port = 0;
        assert!(
            codes(ProfileManager::validate_profile_settings_bounds(&settings).unwrap_err())
                .contains(&"backend_port_out_of_range".to_string())
        );
    }

    #[test]
    fn discord_cooldown_above_24h_is_out_of_range() {
        let mut settings = ProfileSettings::default();
        settings.discord.cooldown_seconds = super::DISCORD_COOLDOWN_SECONDS_MAX + 1;
        assert!(
            codes(ProfileManager::validate_profile_settings_bounds(&settings).unwrap_err())
                .contains(&"discord_cooldown_seconds_out_of_range".to_string())
        );
    }

    #[test]
    fn pii_blocklist_normalization_trims_drops_empties_and_dedupes() {
        let mut entries = vec![
            "  Alice Smith ".to_string(),
            String::new(),
            "   ".to_string(),
            "Alice Smith".to_string(),
            "Springfield".to_string(),
        ];
        super::normalize_pii_blocklist(&mut entries);
        assert_eq!(
            entries,
            vec!["Alice Smith".to_string(), "Springfield".to_string()]
        );
    }
}
