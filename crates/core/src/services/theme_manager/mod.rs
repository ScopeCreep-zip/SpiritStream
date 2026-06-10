//! Theme manager — list, install, hot-reload, and validate themes.
//!
//! Submodules (each is one or more `impl ThemeManager` blocks):
//! - [`catalog`] — list themes from bundled + user dirs, token resolution.
//! - [`install`] — install user theme files; sync bundled themes to user dir.
//! - [`watch`] — filesystem watcher that emits `themes_updated` events.
//! - [`validation`] — theme-file parse, token-set requirements, JSONC depth cap.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use regex::Regex;

use crate::errors::{CoreError, ValidationIssue};

mod catalog;
mod install;
mod validation;
mod watch;

// ---------------------------------------------------------------------------
// Constants shared across submodules.
// ---------------------------------------------------------------------------

pub(super) const THEME_FILE_EXTENSIONS: [&str; 2] = ["json", "jsonc"];
pub(super) const THEME_INSTALL_EXTENSION: &str = "jsonc";
/// JSONC nesting depth cap. `serde_json` itself enforces 128, so this is
/// defense-in-depth tuned to the actual shape of theme files (flat token
/// maps, depth ≤ 3 in practice).
pub(super) const MAX_THEME_JSONC_DEPTH: u32 = 32;
pub(super) const THEME_ID_PATTERN: &str = r"^[a-z0-9][a-z0-9-_]{0,63}$";
pub(super) const THEME_TEMPLATE_NAME: &str = "theme-template.jsonc";
pub(super) const TOKENS_CSS: &str = include_str!("../../../styles/tokens.css");

pub(super) static REQUIRED_TOKENS: OnceLock<Vec<String>> = OnceLock::new();
pub(super) static THEME_ID_REGEX: OnceLock<Regex> = OnceLock::new();

/// How long after a `sync_project_themes` write we ignore matching
/// watcher events. Tuned for the OS file-event latency floor on macOS
/// FSEvents (~250ms) and Linux inotify (~immediate); 2s is a comfortable
/// upper bound that doesn't masquerade as a real user edit.
pub(super) const WATCHER_SELF_FIRE_GRACE: Duration = Duration::from_secs(2);

// ---------------------------------------------------------------------------
// Shared error constructors.
// ---------------------------------------------------------------------------

pub(super) fn theme_invalid(message: impl Into<String>) -> CoreError {
    CoreError::ValidationFailed {
        reasons: vec![ValidationIssue {
            code: "invalid_theme".into(),
            message: message.into(),
            path: None,
        }],
    }
}

pub(super) fn theme_not_found(theme_id: &str) -> CoreError {
    CoreError::ValidationFailed {
        reasons: vec![ValidationIssue {
            code: "theme_not_found".into(),
            message: format!("Theme '{theme_id}' not found"),
            path: Some("/themeId".into()),
        }],
    }
}

// ---------------------------------------------------------------------------
// ThemeManager — public surface.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ThemeManager {
    pub(super) themes_dir: PathBuf,
    pub(super) project_themes_dir: PathBuf,
    /// Audit-log handle for `ThemeValidationFailed` entries. Wired by
    /// `crates/core/src/registry.rs` post-construction via
    /// `set_audit_log`; before that wiring runs (e.g. early-startup
    /// scans during initialization) validation failures still log
    /// WARN but do not audit. Optional rather than required to avoid
    /// a cyclic Arc dependency between AuditLogService and ThemeManager
    /// at construction time.
    pub(super) audit_log: Arc<std::sync::RwLock<Option<Arc<crate::services::AuditLogService>>>>,
    /// Paths the manager just wrote via `sync_project_themes`. The
    /// `notify` watcher fires on its own writes too, which used to
    /// trigger a re-scan + spurious `themes_updated` event on every
    /// boot. This map suppresses watcher events whose path is present
    /// within a short grace window after the write.
    pub(super) recently_synced: Arc<std::sync::Mutex<HashMap<PathBuf, std::time::Instant>>>,
}

impl ThemeManager {
    pub fn new(app_data_dir: PathBuf, project_themes_dir: PathBuf) -> Self {
        let themes_dir = app_data_dir.join("themes");
        if let Err(e) = fs::create_dir_all(&themes_dir) {
            log::warn!("Failed to create themes directory: {e}");
        }
        let absolute_project_themes = if project_themes_dir.is_absolute() {
            project_themes_dir
        } else {
            std::env::current_dir()
                .ok()
                .map(|dir| dir.join(&project_themes_dir))
                .unwrap_or(project_themes_dir)
        };

        log::info!(
            "ThemeManager: project_themes_dir={:?} (exists={})",
            absolute_project_themes,
            absolute_project_themes.exists()
        );
        log::info!("ThemeManager: user themes_dir={themes_dir:?}");

        Self {
            themes_dir,
            project_themes_dir: absolute_project_themes,
            audit_log: Arc::new(std::sync::RwLock::new(None)),
            recently_synced: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Wire the audit-log handle post-construction. Called by
    /// `crates/core/src/registry.rs` after both services exist;
    /// `set_audit_log` is idempotent (a second call replaces the
    /// stored handle). Before this runs, validation failures still
    /// WARN but do not audit.
    pub fn set_audit_log(&self, audit: Arc<crate::services::AuditLogService>) {
        match self.audit_log.write() {
            Ok(mut slot) => *slot = Some(audit),
            Err(e) => {
                log::error!("theme_manager audit_log write lock poisoned during set_audit_log: {e}")
            }
        }
    }
}
