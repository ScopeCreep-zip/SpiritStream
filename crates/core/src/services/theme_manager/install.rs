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
        // I6: route theme install through the atomic writer so the
        // file lands at 0600 (Unix) and a crash mid-install never
        // leaves a partial theme file behind. The themes directory is
        // user-owned but the file is a JSON document the user wrote
        // — same secret-handling pedigree as profile/settings writes.
        crate::services::write_owner_only_atomic(&dest_path, content.as_bytes())?;

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

#[cfg(test)]
mod tests {
    use super::super::validation::required_tokens;
    use super::super::THEME_TEMPLATE_NAME;
    use super::*;
    use tempfile::TempDir;

    /// Build a theme JSONC that satisfies every required token so the
    /// validator accepts it. Each token gets a placeholder colour; the
    /// install path only cares the set is complete and non-empty.
    fn valid_theme_jsonc(id: &str, name: &str, mode: &str) -> String {
        let mut entries = String::new();
        for (i, key) in required_tokens().iter().enumerate() {
            if i > 0 {
                entries.push_str(",\n");
            }
            entries.push_str(&format!("    \"{key}\": \"#101010\""));
        }
        format!(
            "{{\n  \"id\": \"{id}\",\n  \"name\": \"{name}\",\n  \"mode\": \"{mode}\",\n  \"tokens\": {{\n{entries}\n  }}\n}}"
        )
    }

    fn manager(app: &TempDir, proj: &TempDir) -> ThemeManager {
        ThemeManager::new(app.path().to_path_buf(), proj.path().to_path_buf())
    }

    #[test]
    fn install_theme_lands_file_and_returns_summary() {
        let app = TempDir::new().unwrap();
        let proj = TempDir::new().unwrap();
        let mgr = manager(&app, &proj);

        let src = app.path().join("incoming.jsonc");
        fs::write(&src, valid_theme_jsonc("my-theme", "My Theme", "dark")).unwrap();

        let summary = mgr.install_theme(&src).expect("a complete theme installs");
        assert_eq!(summary.id, "my-theme");
        assert_eq!(summary.name, "My Theme");
        assert_eq!(summary.source, "custom");
        assert!(!summary.built_in, "a user theme id is not embedded");

        let dest = app.path().join("themes").join("my-theme.jsonc");
        assert!(dest.exists(), "installed file missing at {dest:?}");
    }

    /// I6: install routes through the atomic owner-only writer, so the
    /// theme file must land at 0600 on Unix.
    #[cfg(unix)]
    #[test]
    fn install_theme_writes_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let app = TempDir::new().unwrap();
        let proj = TempDir::new().unwrap();
        let mgr = manager(&app, &proj);

        let src = app.path().join("incoming.jsonc");
        fs::write(&src, valid_theme_jsonc("perm-theme", "Perm", "light")).unwrap();
        mgr.install_theme(&src).unwrap();

        let dest = app.path().join("themes").join("perm-theme.jsonc");
        let mode = fs::metadata(&dest).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "theme file must be owner-only");
    }

    #[test]
    fn install_theme_rejects_incomplete_token_set() {
        let app = TempDir::new().unwrap();
        let proj = TempDir::new().unwrap();
        let mgr = manager(&app, &proj);

        // Only one token — the validator requires the full set.
        let src = app.path().join("incoming.jsonc");
        fs::write(
            &src,
            r##"{ "id": "bad", "name": "Bad", "mode": "dark", "tokens": { "--bg-base": "#000" } }"##,
        )
        .unwrap();

        let err = mgr.install_theme(&src).unwrap_err();
        assert!(
            matches!(err, CoreError::ValidationFailed { .. }),
            "expected ValidationFailed, got {err:?}",
        );
        assert!(
            !app.path().join("themes").join("bad.jsonc").exists(),
            "a rejected theme must not be written to disk",
        );
    }

    #[test]
    fn install_theme_surfaces_unreadable_source() {
        let app = TempDir::new().unwrap();
        let proj = TempDir::new().unwrap();
        let mgr = manager(&app, &proj);

        let missing = app.path().join("does-not-exist.jsonc");
        let err = mgr.install_theme(&missing).unwrap_err();
        assert!(
            matches!(err, CoreError::Internal { .. }),
            "expected Internal read error, got {err:?}",
        );
    }

    #[test]
    fn sync_copies_real_files_skips_template_and_records_paths() {
        let app = TempDir::new().unwrap();
        let proj = TempDir::new().unwrap();

        fs::write(
            proj.path().join("spirit-dark.jsonc"),
            valid_theme_jsonc("spirit-dark", "Spirit Dark", "dark"),
        )
        .unwrap();
        // The template is a placeholder and must never reach the user dir.
        fs::write(proj.path().join(THEME_TEMPLATE_NAME), "{}").unwrap();

        let mgr = manager(&app, &proj);
        mgr.sync_project_themes();

        let themes_dir = app.path().join("themes");
        let dest = themes_dir.join("spirit-dark.jsonc");
        assert!(
            dest.exists(),
            "bundled theme should be copied to the user dir"
        );
        assert!(
            !themes_dir.join(THEME_TEMPLATE_NAME).exists(),
            "theme-template.jsonc must be skipped",
        );

        // The copied path is recorded so the watcher ignores its own write.
        let synced = mgr.recently_synced.lock().unwrap();
        assert!(
            synced.contains_key(&dest),
            "sync must record the dest path for watcher self-fire suppression",
        );
    }
}
