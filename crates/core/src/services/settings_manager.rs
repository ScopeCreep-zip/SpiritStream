//! SettingsManager — load/save the global `settings.json`.
//!
//! Forward-only architecture (post-rewrite): there is **no migration
//! path** from pre-rewrite settings files. Per-profile data (theme,
//! OBS, Discord, chat, OAuth, backend bind) lives in `ProfileSettings`,
//! not here, and is validated by `ProfileManager`. The global Settings
//! document only carries app-level knobs:
//!
//! * `start_minimized`
//! * `ffmpeg_path` — operator override consumed by `FFmpegLocator`
//! * `log_retention_days` — bounded `[1, 365]`
//! * `last_profile`
//! * `error_reporting_enabled` / `error_reporting_endpoint`
//!
//! Bound checks for fields that moved to `ProfileSettings`
//! (`backend.port`, `discord.cooldown_seconds`) run inside
//! `ProfileManager::save`, not here, because the fields travel with
//! the profile they belong to.

use std::path::PathBuf;
use std::sync::RwLock;

use crate::errors::{CoreError, ValidationIssue};
use crate::models::Settings;

/// Inclusive lower / upper bounds for `log_retention_days`. Below 1
/// disables rotation entirely (a safety footgun for audit-trail use
/// cases); above 365 invites disk-pressure incidents on
/// long-running installs.
pub const LOG_RETENTION_DAYS_MIN: u32 = 1;
pub const LOG_RETENTION_DAYS_MAX: u32 = 365;

/// Manages global settings persistence.
pub struct SettingsManager {
    settings_path: PathBuf,
    cache: RwLock<Option<Settings>>,
}

impl SettingsManager {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let settings_path = app_data_dir.join("settings.json");
        Self {
            settings_path,
            cache: RwLock::new(None),
        }
    }

    /// Load settings from disk, returning `Settings::default()` if the
    /// file is missing. Out-of-range values on load are clamped to
    /// defaults (a startup hang would be worse than a reset).
    pub fn load(&self) -> Result<Settings, CoreError> {
        // Cache fast-path. Poison recovery: a previous writer panicked
        // mid-update — fall through to the disk read rather than trust
        // either the stale value or `Err`.
        match self.cache.read() {
            Ok(cache) => {
                if let Some(ref settings) = *cache {
                    return Ok(settings.clone());
                }
            }
            Err(_) => log::warn!("Settings cache RwLock poisoned on read; falling through to disk"),
        }

        let mut settings: Settings = if self.settings_path.exists() {
            let content =
                std::fs::read_to_string(&self.settings_path).map_err(|e| CoreError::Internal {
                    context: format!("Failed to read settings: {e}"),
                })?;
            // `#[serde(default)]` on every Settings field means a missing
            // key in the user's file gets the default value automatically.
            // No manual merge-missing pass needed.
            serde_json::from_str(&content).map_err(|e| CoreError::Internal {
                context: format!("Failed to parse settings: {e}"),
            })?
        } else {
            let defaults = Settings::default();
            self.save_internal(&defaults)?;
            defaults
        };

        if Self::validate_bounds(&settings).is_err() {
            log::warn!(
                "Persisted settings failed bounds check on load; clamping offending fields to defaults"
            );
            let defaults = Settings::default();
            if settings.log_retention_days < LOG_RETENTION_DAYS_MIN
                || settings.log_retention_days > LOG_RETENTION_DAYS_MAX
            {
                settings.log_retention_days = defaults.log_retention_days;
            }
            let _ = self.save_internal(&settings);
        }

        let mut cache = self.cache.write().unwrap_or_else(|e| e.into_inner());
        *cache = Some(settings.clone());

        Ok(settings)
    }

    /// Save settings to disk after enforcing bound checks. Out-of-range
    /// values produce a single `CoreError::ValidationFailed` carrying
    /// every offending field — callers get a complete list rather than
    /// peeling errors off one at a time.
    pub fn save(&self, settings: &Settings) -> Result<(), CoreError> {
        Self::validate_bounds(settings)?;
        self.save_internal(settings)?;
        let mut cache = self.cache.write().unwrap_or_else(|e| e.into_inner());
        *cache = Some(settings.clone());
        Ok(())
    }

    fn validate_bounds(settings: &Settings) -> Result<(), CoreError> {
        let mut issues = Vec::new();
        if settings.log_retention_days < LOG_RETENTION_DAYS_MIN
            || settings.log_retention_days > LOG_RETENTION_DAYS_MAX
        {
            issues.push(ValidationIssue {
                code: "log_retention_days_out_of_range".into(),
                message: format!(
                    "log_retention_days must be in [{LOG_RETENTION_DAYS_MIN}, {LOG_RETENTION_DAYS_MAX}], got {}",
                    settings.log_retention_days
                ),
                path: Some("/logRetentionDays".into()),
            });
        }
        if issues.is_empty() {
            Ok(())
        } else {
            Err(CoreError::ValidationFailed { reasons: issues })
        }
    }

    fn save_internal(&self, settings: &Settings) -> Result<(), CoreError> {
        if let Some(parent) = self.settings_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| CoreError::Internal {
                context: format!("Failed to create settings directory: {e}"),
            })?;
        }
        let content = serde_json::to_string_pretty(settings).map_err(|e| CoreError::Internal {
            context: format!("Failed to serialize settings: {e}"),
        })?;
        // settings.json may carry the operator API token in
        // some deployments, so always write owner-only.
        crate::services::write_owner_only_atomic(&self.settings_path, content.as_bytes())
    }

    pub fn get_profiles_path(&self) -> PathBuf {
        self.settings_path
            .parent()
            .map(|p| p.join("profiles"))
            .unwrap_or_else(|| PathBuf::from("profiles"))
    }

    /// Export settings + profiles to a directory, capped at 100 MiB.
    /// Refuses upfront if the source already exceeds the
    /// cap, and re-checks after each copy so a profiles dir with
    /// millions of files cannot drain the host disk via a runaway
    /// export.
    pub fn export_data(&self, export_path: &PathBuf) -> Result<(), CoreError> {
        const MAX_EXPORT_BYTES: u64 = 100 * 1024 * 1024;
        let mut total_bytes: u64 = 0;
        let too_large = |total: u64, more: u64| total.saturating_add(more) > MAX_EXPORT_BYTES;
        let oversize = || CoreError::Internal {
            context: format!("export exceeds {MAX_EXPORT_BYTES}-byte cap"),
        };

        std::fs::create_dir_all(export_path).map_err(|e| CoreError::Internal {
            context: format!("Failed to create export directory: {e}"),
        })?;

        if self.settings_path.exists() {
            let size = std::fs::metadata(&self.settings_path)
                .map_err(|e| CoreError::Internal {
                    context: e.to_string(),
                })?
                .len();
            if too_large(total_bytes, size) {
                return Err(oversize());
            }
            total_bytes += size;
            let dest = export_path.join("settings.json");
            std::fs::copy(&self.settings_path, &dest).map_err(|e| CoreError::Internal {
                context: format!("Failed to export settings: {e}"),
            })?;
        }

        let profiles_dir = self.get_profiles_path();
        if profiles_dir.exists() {
            let export_profiles_dir = export_path.join("profiles");
            std::fs::create_dir_all(&export_profiles_dir).map_err(|e| CoreError::Internal {
                context: format!("Failed to create profiles export directory: {e}"),
            })?;

            for entry in std::fs::read_dir(&profiles_dir)
                .map_err(|e| CoreError::Internal {
                    context: e.to_string(),
                })?
                .flatten()
            {
                let size = entry
                    .metadata()
                    .map_err(|e| CoreError::Internal {
                        context: e.to_string(),
                    })?
                    .len();
                if too_large(total_bytes, size) {
                    return Err(oversize());
                }
                total_bytes += size;
                let dest = export_profiles_dir.join(entry.file_name());
                std::fs::copy(entry.path(), dest).map_err(|e| CoreError::Internal {
                    context: format!("Failed to export profile: {e}"),
                })?;
            }
        }

        Ok(())
    }

    /// Clear settings + every profile from disk. Used by
    /// the destructive "reset app" flow gated behind a one-shot
    /// confirm token.
    pub fn clear_data(&self) -> Result<(), CoreError> {
        if self.settings_path.exists() {
            std::fs::remove_file(&self.settings_path).map_err(|e| CoreError::Internal {
                context: format!("Failed to remove settings: {e}"),
            })?;
        }
        let profiles_dir = self.get_profiles_path();
        if profiles_dir.exists() {
            std::fs::remove_dir_all(&profiles_dir).map_err(|e| CoreError::Internal {
                context: format!("Failed to remove profiles: {e}"),
            })?;
        }
        // Drop the cache so subsequent loads reread from a (now empty)
        // disk and write fresh defaults.
        if let Ok(mut cache) = self.cache.write() {
            *cache = None;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn manager() -> (SettingsManager, TempDir) {
        let tmp = TempDir::new().expect("tempdir");
        let mgr = SettingsManager::new(tmp.path().to_path_buf());
        (mgr, tmp)
    }

    #[test]
    fn load_writes_and_returns_defaults_when_missing() {
        let (mgr, tmp) = manager();
        let settings = mgr.load().expect("load defaults");
        assert_eq!(settings.log_retention_days, 30);
        assert!(tmp.path().join("settings.json").exists());
    }

    #[test]
    fn save_rejects_log_retention_below_min() {
        let (mgr, _tmp) = manager();
        let mut s = mgr.load().unwrap();
        s.log_retention_days = 0;
        match mgr.save(&s) {
            Err(CoreError::ValidationFailed { reasons }) => {
                assert!(reasons
                    .iter()
                    .any(|r| r.code == "log_retention_days_out_of_range"));
            }
            other => panic!("expected ValidationFailed, got {other:?}"),
        }
    }

    #[test]
    fn save_rejects_log_retention_above_max() {
        let (mgr, _tmp) = manager();
        let mut s = mgr.load().unwrap();
        s.log_retention_days = 366;
        assert!(matches!(
            mgr.save(&s),
            Err(CoreError::ValidationFailed { .. })
        ));
    }

    #[test]
    fn save_accepts_valid_bounds_round_trip() {
        let (mgr, _tmp) = manager();
        let mut s = mgr.load().unwrap();
        s.log_retention_days = 90;
        mgr.save(&s).expect("valid bounds should save");
        // New manager re-reads from disk (bypasses the cache).
        let fresh = SettingsManager::new(mgr.settings_path.parent().unwrap().to_path_buf());
        let reloaded = fresh.load().unwrap();
        assert_eq!(reloaded.log_retention_days, 90);
    }

    /// Every successful settings save must land at mode
    /// 0600 on Unix. Regression-tested at the manager level (not just
    /// the secure_io helper) because settings.json may carry the
    /// operator API token in some deployments.
    #[cfg(unix)]
    #[test]
    fn save_writes_settings_json_at_0600() {
        use std::os::unix::fs::PermissionsExt;
        let (mgr, _tmp) = manager();
        let s = mgr.load().expect("load defaults");
        mgr.save(&s).expect("save defaults");
        let perms = std::fs::metadata(&mgr.settings_path).unwrap().permissions();
        assert_eq!(
            perms.mode() & 0o777,
            0o600,
            "settings.json must be owner-only after save",
        );
    }
}
