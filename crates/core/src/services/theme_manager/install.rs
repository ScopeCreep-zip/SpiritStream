//! Theme install + sync helpers.
//!
//! `install_theme` lands a single user-supplied JSONC at the
//! conventional `{themes_dir}/{theme.id}.jsonc` path and emits a
//! `ThemeSummary` for the UI. `sync_project_themes` copies the
//! bundled themes from `project_themes_dir` into the user dir on
//! startup; both paths recorded in `recently_synced` so the
//! watcher loop knows to ignore its own writes.

use std::fs;
use std::path::Path;

use crate::errors::CoreError;
use crate::models::ThemeSummary;

use super::catalog::should_skip_theme_file;
use super::validation::{apply_token_fallbacks, parse_theme, validate_theme};
use super::{ThemeManager, THEME_INSTALL_EXTENSION};

impl ThemeManager {
    /// Syncs built-in/project themes to the appdata themes directory,
    /// skipping `theme-template.jsonc`.
    ///
    /// In production: Uses Tauri's resource_dir to access bundled themes.
    /// In development: Falls back to `../themes` relative path.
    pub fn sync_project_themes(&self) {
        log::info!(
            "Syncing themes from {:?} to {:?}",
            self.project_themes_dir,
            self.themes_dir
        );

        match fs::read_dir(&self.project_themes_dir) {
            Ok(entries) => {
                let mut copied = 0;
                for entry in entries.flatten() {
                    let path = entry.path();
                    if should_skip_theme_file(&path) {
                        continue;
                    }
                    let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    let dest_path = self.themes_dir.join(fname);
                    match fs::copy(&path, &dest_path) {
                        Ok(_) => {
                            log::info!("Synced theme {fname:?} to appdata");
                            copied += 1;
                            // Tell the watcher loop to ignore the inotify /
                            // FSEvent the OS will deliver for this just-written
                            // file. Without this, the watcher fires a spurious
                            // `themes_updated` event on every boot.
                            if let Ok(mut map) = self.recently_synced.lock() {
                                map.insert(dest_path.clone(), std::time::Instant::now());
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to sync theme {fname:?} to appdata: {e}");
                        }
                    }
                }
                log::info!("Theme sync complete: {copied} theme(s) copied");
            }
            Err(e) => {
                log::error!(
                    "Failed to read project themes directory {:?}: {e}",
                    self.project_themes_dir
                );
            }
        }
    }

    pub fn install_theme(&self, source_path: &Path) -> Result<ThemeSummary, CoreError> {
        let content = fs::read_to_string(source_path).map_err(|e| CoreError::Internal {
            context: format!("Failed to read theme file: {e}"),
        })?;
        let mut theme = parse_theme(&content)?;
        apply_token_fallbacks(&mut theme.tokens);

        validate_theme(&theme)?;

        fs::create_dir_all(&self.themes_dir).map_err(|e| CoreError::Internal {
            context: format!("Failed to create themes directory: {e}"),
        })?;

        let dest_path = self
            .themes_dir
            .join(format!("{}.{}", theme.id, THEME_INSTALL_EXTENSION));
        fs::write(&dest_path, content).map_err(|e| CoreError::Internal {
            context: format!("Failed to copy theme file: {e}"),
        })?;

        let built_in = crate::services::is_embedded_theme(&theme.id);
        Ok(ThemeSummary {
            id: theme.id,
            name: theme.name,
            mode: theme.mode,
            source: "custom".to_string(),
            built_in,
            valid: true,
            error: None,
        })
    }
}
