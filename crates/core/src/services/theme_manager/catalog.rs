//! Theme catalog — list themes and resolve token maps.
//!
//! Reads from bundled themes (`project_themes_dir`), the embedded fallback,
//! and the user's themes dir. No silent fallback chain in `get_theme_tokens`
//! — a broken user override is reported, not swallowed.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::CoreError;
use crate::models::{ThemeFile, ThemeSummary};

use super::validation::load_theme_file;
use super::{theme_invalid, theme_not_found, ThemeManager, THEME_FILE_EXTENSIONS, THEME_TEMPLATE_NAME};

impl ThemeManager {
    pub fn list_themes(&self) -> Vec<ThemeSummary> {
        // Sync is now only done on app startup and when refresh_themes() is
        // called. Calling sync from here would deadlock with the file watcher.
        let mut themes = Vec::new();
        let mut seen_ids = HashSet::new();

        self.append_themes_from_dir(
            &self.project_themes_dir,
            "builtin",
            &mut themes,
            &mut seen_ids,
        );

        if themes.is_empty() {
            log::info!(
                "No themes found in project_themes_dir, using embedded theme list as fallback"
            );
            for summary in crate::services::embedded_themes::get_embedded_theme_list() {
                if !seen_ids.contains(&summary.id) {
                    seen_ids.insert(summary.id.clone());
                    themes.push(summary);
                }
            }
        }

        self.append_themes_from_dir(&self.themes_dir, "custom", &mut themes, &mut seen_ids);

        themes
    }

    pub fn get_theme_tokens(&self, theme_id: &str) -> Result<HashMap<String, String>, CoreError> {
        // Resolution order — no silent fallback chain.
        //
        // 1. **User override**: a file at `{themes_dir}/{theme_id}.jsonc`
        //    is the operator's claim about `theme_id`. If it loads
        //    cleanly, use it. If it loads broken, fail loud — silently
        //    reverting to the bundled copy would lie to the operator
        //    about whether their edits applied.
        // 2. **Bundled-embedded**: if the theme id matches a built-in,
        //    return the embedded tokens. This is the canonical source
        //    for bundled themes, not a fallback.
        // 3. **Filesystem bundled** (dev tree's `themes/` dir): only
        //    relevant in dev where embedded tokens may be stale; same
        //    fail-loud rule as the user dir.
        // 4. None of the above → `theme_not_found`.
        if let Some(theme) = self.find_theme_in_dir(theme_id, &self.themes_dir)? {
            log::info!(
                "get_theme_tokens('{theme_id}'): user override ({} tokens)",
                theme.tokens.len()
            );
            return Ok(theme.tokens);
        }
        if let Some(tokens) = crate::services::embedded_themes::get_embedded_theme_tokens(theme_id) {
            log::info!(
                "get_theme_tokens('{theme_id}'): bundled embedded ({} tokens)",
                tokens.len()
            );
            return Ok(tokens);
        }
        if let Some(theme) = self.find_theme_in_dir(theme_id, &self.project_themes_dir)? {
            log::info!(
                "get_theme_tokens('{theme_id}'): dev project_themes_dir ({} tokens)",
                theme.tokens.len()
            );
            return Ok(theme.tokens);
        }
        Err(theme_not_found(theme_id))
    }

    pub(super) fn append_themes_from_dir(
        &self,
        dir: &Path,
        source: &str,
        themes: &mut Vec<ThemeSummary>,
        seen_ids: &mut HashSet<String>,
    ) {
        if !dir.exists() {
            return;
        }

        for path in theme_paths_from_dir(dir) {
            if should_skip_theme_file(&path) {
                continue;
            }

            match load_theme_file(&path) {
                Ok(theme) => {
                    if seen_ids.contains(&theme.id) {
                        continue;
                    }
                    seen_ids.insert(theme.id.clone());
                    let built_in = crate::services::is_embedded_theme(&theme.id);
                    themes.push(ThemeSummary {
                        id: theme.id,
                        name: theme.name,
                        mode: theme.mode,
                        source: source.to_string(),
                        built_in,
                        valid: true,
                        error: None,
                    });
                }
                Err(error) => {
                    // Broken theme — surface it in the list with
                    // `valid: false` so the operator sees their file
                    // is malformed instead of it silently vanishing.
                    // ID is recovered from the filename stem (the
                    // installer's naming convention).
                    let stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    if seen_ids.contains(&stem) {
                        continue;
                    }
                    seen_ids.insert(stem.clone());
                    let err_msg = error.to_string();
                    let file_name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    log::warn!("Invalid theme file {file_name:?}: {err_msg}");

                    // Audit-log entry so operators get a structured,
                    // queryable signal beyond the transient WARN line.
                    // Especially relevant for accessibility regressions
                    // (e.g. high-contrast theme missing tokens — silent
                    // fallback to default for low-vision users).
                    if let Ok(slot) = self.audit_log.read() {
                        if let Some(audit) = slot.as_ref() {
                            let _ = audit.record(
                                crate::services::AuditAction::ThemeValidationFailed {
                                    file_name: file_name.clone(),
                                    reason: err_msg.clone(),
                                },
                            );
                        }
                    }
                    themes.push(ThemeSummary {
                        id: stem.clone(),
                        name: stem,
                        mode: crate::models::ThemeMode::Dark,
                        source: source.to_string(),
                        built_in: false,
                        valid: false,
                        error: Some(err_msg),
                    });
                }
            }
        }
    }

    /// Find a theme in a directory.
    /// - `Ok(Some(theme))` — file found and parsed successfully.
    /// - `Ok(None)` — directory exists but no file claims `theme_id`.
    /// - `Err(CoreError)` — a file at the conventional path
    ///   `{dir}/{theme_id}.jsonc` (the install layout) exists but
    ///   failed to parse. Caller propagates rather than silently
    ///   falling through to the bundled copy, so the operator sees
    ///   their broken theme instead of a phantom revert.
    pub(super) fn find_theme_in_dir(
        &self,
        theme_id: &str,
        dir: &Path,
    ) -> Result<Option<ThemeFile>, CoreError> {
        if !dir.exists() {
            log::debug!("find_theme_in_dir: directory does not exist: {dir:?}");
            return Ok(None);
        }

        // Conventional path first: installed themes always land at
        // `{themes_dir}/{theme.id}.jsonc` (see `install_theme`). If a
        // file matches that path, it is the user's claim about
        // `theme_id` — load failures here are reported, not swallowed.
        for ext in ["jsonc", "json"] {
            let direct = dir.join(format!("{theme_id}.{ext}"));
            if direct.exists() && !should_skip_theme_file(&direct) {
                return match load_theme_file(&direct) {
                    Ok(theme) if theme.id == theme_id => {
                        log::info!("find_theme_in_dir: FOUND theme '{theme_id}' at {direct:?}");
                        Ok(Some(theme))
                    }
                    Ok(theme) => Err(theme_invalid(format!(
                        "Theme file {direct:?} declares id '{}', not '{theme_id}'",
                        theme.id
                    ))),
                    Err(e) => Err(theme_invalid(format!(
                        "Theme file {direct:?} failed to load: {e}"
                    ))),
                };
            }
        }

        // Fallback scan for non-conventionally-named files (older
        // installs, manually-placed themes). A parse error during the
        // scan stays a debug log — the file isn't claiming `theme_id`
        // by convention, so we can't know it was meant to match.
        let paths = theme_paths_from_dir(dir);
        for path in paths {
            if should_skip_theme_file(&path) {
                continue;
            }
            match load_theme_file(&path) {
                Ok(theme) if theme.id == theme_id => {
                    log::info!("find_theme_in_dir: FOUND theme '{theme_id}' at {path:?}");
                    return Ok(Some(theme));
                }
                Ok(_) => {}
                Err(e) => log::debug!("find_theme_in_dir: failed to load {path:?}: {e}"),
            }
        }

        log::debug!("find_theme_in_dir: theme '{theme_id}' NOT found in {dir:?}");
        Ok(None)
    }
}

pub(super) fn theme_paths_from_dir(dir: &Path) -> Vec<PathBuf> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    paths.sort_by_key(|path| match path.extension().and_then(|s| s.to_str()) {
        Some("jsonc") => 0,
        Some("json") => 1,
        _ => 2,
    });

    paths
}

pub(super) fn should_skip_theme_file(path: &Path) -> bool {
    if !is_theme_file(path) {
        return true;
    }
    let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    fname == THEME_TEMPLATE_NAME || fname.contains(".deprecated")
}

fn is_theme_file(path: &Path) -> bool {
    match path.extension().and_then(|s| s.to_str()) {
        Some(ext) => THEME_FILE_EXTENSIONS.contains(&ext),
        None => false,
    }
}
