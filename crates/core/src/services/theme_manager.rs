use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::Duration;

use crate::services::{emit_event, EventSink};
use notify::{RecursiveMode, Watcher};
use regex::Regex;

use crate::errors::{CoreError, ValidationIssue};
use crate::models::{ThemeFile, ThemeSummary};

fn theme_invalid(message: impl Into<String>) -> CoreError {
    CoreError::ValidationFailed {
        reasons: vec![ValidationIssue {
            code: "invalid_theme".into(),
            message: message.into(),
            path: None,
        }],
    }
}

fn theme_not_found(theme_id: &str) -> CoreError {
    CoreError::ValidationFailed {
        reasons: vec![ValidationIssue {
            code: "theme_not_found".into(),
            message: format!("Theme '{theme_id}' not found"),
            path: Some("/themeId".into()),
        }],
    }
}

const THEME_FILE_EXTENSIONS: [&str; 2] = ["json", "jsonc"];
const THEME_INSTALL_EXTENSION: &str = "jsonc";
/// JSONC nesting depth cap. `serde_json` itself enforces
/// 128, so this is defense-in-depth tuned to the actual shape of theme
/// files (flat token maps, depth ≤ 3 in practice).
const MAX_THEME_JSONC_DEPTH: u32 = 32;
const THEME_ID_PATTERN: &str = r"^[a-z0-9][a-z0-9-_]{0,63}$";
const THEME_TEMPLATE_NAME: &str = "theme-template.jsonc";
const TOKENS_CSS: &str = include_str!("../../styles/tokens.css");

static REQUIRED_TOKENS: OnceLock<Vec<String>> = OnceLock::new();
static THEME_ID_REGEX: OnceLock<Regex> = OnceLock::new();

#[derive(Clone)]
pub struct ThemeManager {
    themes_dir: PathBuf,
    project_themes_dir: PathBuf,
    /// Audit-log handle for `ThemeValidationFailed` entries. Wired by
    /// `crates/core/src/registry.rs` post-construction via
    /// `set_audit_log`; before that wiring runs (e.g. early-startup
    /// scans during initialization) validation failures still log
    /// WARN but do not audit. Optional rather than required to avoid
    /// a cyclic Arc dependency between AuditLogService and ThemeManager
    /// at construction time.
    audit_log: Arc<std::sync::RwLock<Option<Arc<crate::services::AuditLogService>>>>,
    /// Paths the manager just wrote via `sync_project_themes`. The
    /// `notify` watcher fires on its own writes too, which used to
    /// trigger a re-scan + spurious `themes_updated` event on every
    /// boot. This map suppresses watcher events whose path is present
    /// within a short grace window after the write.
    recently_synced: Arc<std::sync::Mutex<HashMap<PathBuf, std::time::Instant>>>,
}

/// How long after a `sync_project_themes` write we ignore matching
/// watcher events. Tuned for the OS file-event latency floor on macOS
/// FSEvents (~250ms) and Linux inotify (~immediate); 2s is a comfortable
/// upper bound that doesn't masquerade as a real user edit.
const WATCHER_SELF_FIRE_GRACE: Duration = Duration::from_secs(2);

impl ThemeManager {
    /// Syncs built-in/project themes to the appdata themes directory, skipping theme-template.jsonc
    ///
    /// In production: Uses Tauri's resource_dir to access bundled themes
    /// In development: Falls back to ../themes relative path
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

        // Log paths for debugging theme loading issues
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
        if let Ok(mut slot) = self.audit_log.write() {
            *slot = Some(audit);
        }
    }

    pub fn list_themes(&self) -> Vec<ThemeSummary> {
        // Note: sync is now only done on app startup and when refresh_themes() is called
        // This prevents infinite loops with the file watcher
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
            for summary in super::embedded_themes::get_embedded_theme_list() {
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
        if let Some(tokens) = super::embedded_themes::get_embedded_theme_tokens(theme_id) {
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

    pub fn install_theme(&self, source_path: &Path) -> Result<ThemeSummary, CoreError> {
        let content = fs::read_to_string(source_path).map_err(|e| CoreError::Internal {
            context: format!("Failed to read theme file: {e}"),
        })?;
        let mut theme = Self::parse_theme(&content)?;
        Self::apply_token_fallbacks(&mut theme.tokens);

        Self::validate_theme(&theme)?;

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

    pub fn start_watcher(&self, event_sink: Arc<dyn EventSink>) {
        let manager = self.clone();
        let themes_dir = self.themes_dir.clone();
        let event_sink = Arc::clone(&event_sink);
        thread::spawn(move || {
            let (tx, rx) = std::sync::mpsc::channel();
            let mut watcher = match notify::recommended_watcher(tx) {
                Ok(watcher) => watcher,
                Err(error) => {
                    log::warn!("Theme watcher failed to start: {error}");
                    return;
                }
            };

            if let Err(error) = watcher.watch(&themes_dir, RecursiveMode::NonRecursive) {
                log::warn!("Failed to watch themes directory: {error}");
                return;
            }

            let mut last_update = std::time::Instant::now();
            for event in rx {
                let event = match event {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                // Suppress events whose paths were just written by
                // `sync_project_themes`. The OS delivers an inotify /
                // FSEvent for self-writes too — without this filter,
                // every boot triggers a spurious `themes_updated` push
                // to the WebSocket that the React side then re-renders
                // for no reason.
                let now = std::time::Instant::now();
                let from_self = if let Ok(mut map) = manager.recently_synced.lock() {
                    // Expire entries past the grace window first so the
                    // map doesn't grow without bound.
                    map.retain(|_, t| now.duration_since(*t) < WATCHER_SELF_FIRE_GRACE);
                    !event.paths.is_empty()
                        && event.paths.iter().all(|p| {
                            map.get(p)
                                .is_some_and(|t| now.duration_since(*t) < WATCHER_SELF_FIRE_GRACE)
                        })
                } else {
                    false
                };
                if from_self {
                    continue;
                }

                // Debounce: only emit theme updates at most once per second
                if now.duration_since(last_update) < Duration::from_secs(1) {
                    continue;
                }
                last_update = now;

                let themes = manager.list_themes();
                emit_event(event_sink.as_ref(), "themes_updated", &themes);
            }
        });
    }

    fn append_themes_from_dir(
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

            match Self::load_theme_file(&path) {
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
    fn find_theme_in_dir(
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
                return match Self::load_theme_file(&direct) {
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
            match Self::load_theme_file(&path) {
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

    fn load_theme_file(path: &Path) -> Result<ThemeFile, CoreError> {
        let content = fs::read_to_string(path).map_err(|e| CoreError::Internal {
            context: format!("Failed to read theme: {e}"),
        })?;
        let mut theme = Self::parse_theme(&content)?;
        Self::apply_token_fallbacks(&mut theme.tokens);
        Self::validate_theme(&theme)?;
        Ok(theme)
    }

    fn parse_theme(content: &str) -> Result<ThemeFile, CoreError> {
        let sanitized = strip_jsonc_comments(content);
        // Reject inputs whose nesting depth exceeds the
        // policy cap before handing them to `serde_json`. Themes are
        // flat token maps (depth 1-3 in practice). `serde_json` itself
        // bounds recursion at 128, so this is defense-in-depth that
        // catches stack-blow attempts well before the parser does.
        if let Some(depth) = max_brace_depth_or_overflow(&sanitized, MAX_THEME_JSONC_DEPTH) {
            return Err(theme_invalid(format!(
                "Theme JSONC nesting depth {depth} exceeds limit {MAX_THEME_JSONC_DEPTH}"
            )));
        }
        serde_json::from_str(&sanitized)
            .map_err(|e| theme_invalid(format!("Invalid theme JSON: {e}")))
    }

    fn apply_token_fallbacks(tokens: &mut HashMap<String, String>) {
        if !tokens.contains_key("--border-subtle") {
            if let Some(border_muted) = tokens.get("--border-muted").cloned() {
                tokens.insert("--border-subtle".to_string(), border_muted);
            }
        }
    }

    fn validate_theme(theme: &ThemeFile) -> Result<(), CoreError> {
        if theme.id.trim().is_empty() {
            return Err(theme_invalid("Theme id is required"));
        }
        if theme.name.trim().is_empty() {
            return Err(theme_invalid("Theme name is required"));
        }

        let id_regex = THEME_ID_REGEX.get_or_init(|| Regex::new(THEME_ID_PATTERN).unwrap());
        if !id_regex.is_match(&theme.id) {
            return Err(theme_invalid(
                "Theme id must be lowercase alphanumeric with dashes or underscores",
            ));
        }

        if theme.tokens.is_empty() {
            return Err(theme_invalid("Theme tokens cannot be empty"));
        }

        let required = required_tokens();
        let mode_label = theme.mode.as_str();
        Self::validate_token_set(mode_label, &theme.tokens, required)?;

        Ok(())
    }

    fn validate_token_set(
        label: &str,
        tokens: &HashMap<String, String>,
        required: &[String],
    ) -> Result<(), CoreError> {
        let missing: Vec<&String> = required
            .iter()
            .filter(|key| !tokens.contains_key(*key))
            .collect();
        if !missing.is_empty() {
            let preview = missing
                .iter()
                .take(5)
                .map(|k| (*k).as_str())
                .collect::<Vec<_>>();
            let remaining = missing.len().saturating_sub(preview.len());
            let suffix = if remaining > 0 {
                format!(" (and {remaining} more)")
            } else {
                "".to_string()
            };

            return Err(theme_invalid(format!(
                "Missing {label} tokens: {}{suffix}",
                preview.join(", ")
            )));
        }

        for (key, value) in tokens {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(theme_invalid(format!(
                    "Invalid {label} token '{key}': value cannot be empty"
                )));
            }
            if trimmed.contains("REPLACE_ME") {
                return Err(theme_invalid(format!(
                    "Invalid {label} token '{key}': replace REPLACE_ME placeholders"
                )));
            }
            if trimmed.contains("</style>") || trimmed.contains("<script") {
                return Err(theme_invalid(format!(
                    "Invalid {label} token '{key}': value contains dangerous content"
                )));
            }
        }

        Ok(())
    }
}

fn required_tokens() -> &'static Vec<String> {
    REQUIRED_TOKENS.get_or_init(|| {
        let token_regex = Regex::new(r"--[A-Za-z0-9_-]+").unwrap();
        let mut tokens = HashSet::new();

        // Extract all tokens from tokens.css
        for cap in token_regex.captures_iter(TOKENS_CSS) {
            if let Some(matched) = cap.get(0) {
                let token = matched.as_str();

                // Skip optional tokens that themes can customize or omit
                // Color scales: violet-*, fuchsia-*, pink-*, neutral-*, purple-*, cyan-*, green-*, orange-*, red-*, yellow-*
                // Typography: font-*, letter-spacing-*, line-height-*
                let is_optional = token.starts_with("--violet-")
                    || token.starts_with("--fuchsia-")
                    || token.starts_with("--pink-")
                    || token.starts_with("--neutral-")
                    || token.starts_with("--purple-")
                    || token.starts_with("--cyan-")
                    || token.starts_with("--green-")
                    || token.starts_with("--orange-")
                    || token.starts_with("--red-")
                    || token.starts_with("--yellow-")
                    || token.starts_with("--font-")
                    || token.starts_with("--letter-spacing-")
                    || token.starts_with("--line-height-");

                // Only require core semantic tokens
                if !is_optional {
                    tokens.insert(token.to_string());
                }
            }
        }

        let mut tokens: Vec<String> = tokens.into_iter().collect();
        tokens.sort();
        tokens
    })
}

fn theme_paths_from_dir(dir: &Path) -> Vec<PathBuf> {
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

fn should_skip_theme_file(path: &Path) -> bool {
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

/// Scan `input` and return the maximum brace/bracket nesting depth, or
/// `None` if depth never exceeds `limit`. String literals are skipped so
/// `{` inside a string doesn't count. Used to reject
/// stack-blow attempts before they reach `serde_json`.
fn max_brace_depth_or_overflow(input: &str, limit: u32) -> Option<u32> {
    let mut depth: u32 = 0;
    let mut max_seen: u32 = 0;
    let mut in_string = false;
    let mut escape = false;
    for byte in input.bytes() {
        if in_string {
            if escape {
                escape = false;
            } else if byte == b'\\' {
                escape = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth = depth.saturating_add(1);
                if depth > max_seen {
                    max_seen = depth;
                }
                if depth > limit {
                    return Some(depth);
                }
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    None
}

fn strip_jsonc_comments(input: &str) -> String {
    let input = input.strip_prefix('\u{FEFF}').unwrap_or(input);
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escape = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while let Some(ch) = chars.next() {
        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
                output.push(ch);
            }
            continue;
        }

        if in_block_comment {
            if ch == '*' {
                if let Some('/') = chars.peek() {
                    chars.next();
                    in_block_comment = false;
                }
                continue;
            }
            if ch == '\n' {
                output.push(ch);
            }
            continue;
        }

        if in_string {
            output.push(ch);
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            output.push(ch);
            continue;
        }

        if ch == '/' {
            match chars.peek() {
                Some('/') => {
                    chars.next();
                    in_line_comment = true;
                    continue;
                }
                Some('*') => {
                    chars.next();
                    in_block_comment = true;
                    continue;
                }
                _ => {}
            }
        }

        output.push(ch);
    }

    output
}

#[cfg(test)]
mod phase_6_10_tests {
    use super::*;

    #[test]
    fn flat_theme_passes_depth_check() {
        let theme = r#"{ "id": "x", "tokens": { "a": "b", "c": "d" } }"#;
        assert_eq!(
            max_brace_depth_or_overflow(theme, MAX_THEME_JSONC_DEPTH),
            None
        );
    }

    #[test]
    fn deeply_nested_input_is_rejected() {
        let mut payload = String::new();
        for _ in 0..40 {
            payload.push('{');
        }
        payload.push_str("\"x\":1");
        for _ in 0..40 {
            payload.push('}');
        }
        let result = max_brace_depth_or_overflow(&payload, MAX_THEME_JSONC_DEPTH);
        assert!(
            matches!(result, Some(d) if d > MAX_THEME_JSONC_DEPTH),
            "40-deep input must exceed the {MAX_THEME_JSONC_DEPTH} cap",
        );
    }

    #[test]
    fn braces_inside_strings_dont_count() {
        // `{` inside a JSON string literal must NOT inflate the depth.
        // 40 inside a string, only 1 real outer object.
        let mut payload = String::from("{ \"value\": \"");
        for _ in 0..40 {
            payload.push('{');
        }
        payload.push_str("\" }");
        assert_eq!(
            max_brace_depth_or_overflow(&payload, MAX_THEME_JSONC_DEPTH),
            None
        );
    }

    #[test]
    fn parse_theme_rejects_oversized_nesting() {
        let mut payload = String::new();
        for _ in 0..40 {
            payload.push('{');
        }
        payload.push_str("\"x\":1");
        for _ in 0..40 {
            payload.push('}');
        }
        let err = ThemeManager::parse_theme(&payload).unwrap_err();
        let CoreError::ValidationFailed { reasons } = err else {
            panic!("expected ValidationFailed, got {err:?}");
        };
        assert!(
            reasons.iter().any(|r| r.message.contains("nesting depth")),
            "expected depth error in reasons, got {reasons:?}",
        );
    }
}
