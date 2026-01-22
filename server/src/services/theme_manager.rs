use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use regex::Regex;
use crate::services::{emit_event, EventSink};

use crate::models::{ThemeFile, ThemeSummary};

const THEME_FILE_EXTENSIONS: [&str; 2] = ["json", "jsonc"];
const THEME_INSTALL_EXTENSION: &str = "jsonc";
const THEME_ID_PATTERN: &str = r"^[a-z0-9][a-z0-9-_]{0,63}$";
const THEME_TEMPLATE_NAME: &str = "theme-template.jsonc";
const TOKENS_CSS: &str = include_str!("../../styles/tokens.css");

static REQUIRED_TOKENS: OnceLock<Vec<String>> = OnceLock::new();
static THEME_ID_REGEX: OnceLock<Regex> = OnceLock::new();

#[derive(Clone)]
pub struct ThemeManager {
    themes_dir: PathBuf,
    project_themes_dir: PathBuf,
}

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
                            log::info!("Synced theme {:?} to appdata", fname);
                            copied += 1;
                        }
                        Err(e) => {
                            log::error!("Failed to sync theme {:?} to appdata: {e}", fname);
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
        log::info!("ThemeManager: user themes_dir={:?}", themes_dir);

        Self {
            themes_dir,
            project_themes_dir: absolute_project_themes,
        }
    }

    pub fn list_themes(&self) -> Vec<ThemeSummary> {
        // Note: sync is now only done on app startup and when refresh_themes() is called
        // This prevents infinite loops with the file watcher
        let mut themes = Vec::new();
        let mut seen_ids = HashSet::new();

        self.append_themes_from_dir(&self.project_themes_dir, "builtin", &mut themes, &mut seen_ids);

        if themes.is_empty() {
            log::info!("No themes found in project_themes_dir, using embedded theme list as fallback");
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

    pub fn get_theme_tokens(&self, theme_id: &str) -> Result<HashMap<String, String>, String> {
        log::info!(
            "get_theme_tokens('{}') - searching directories:",
            theme_id
        );
        log::info!(
            "  user themes_dir: {:?} (exists={})",
            self.themes_dir,
            self.themes_dir.exists()
        );
        log::info!(
            "  project_themes_dir: {:?} (exists={})",
            self.project_themes_dir,
            self.project_themes_dir.exists()
        );

        // FIRST: Try to load from theme files (normal path)
        match self.find_theme_by_id(theme_id) {
            Ok(theme) => {
                log::info!(
                    "Found theme '{}' from file with {} tokens",
                    theme_id,
                    theme.tokens.len()
                );
                return Ok(theme.tokens);
            }
            Err(e) => {
                log::warn!("Theme file not found for '{}': {}", theme_id, e);
            }
        }

        // SECOND: Check embedded themes (fallback for production builds)
        if let Some(tokens) = super::embedded_themes::get_embedded_theme_tokens(theme_id) {
            log::info!(
                "Using embedded fallback for theme '{}' ({} tokens)",
                theme_id,
                tokens.len()
            );
            return Ok(tokens);
        }

        // THIRD: Theme not found anywhere
        Err(format!(
            "Theme '{}' not found in files or embedded themes",
            theme_id
        ))
    }

    pub fn install_theme(&self, source_path: &Path) -> Result<ThemeSummary, String> {
        let content = fs::read_to_string(source_path)
            .map_err(|e| format!("Failed to read theme file: {e}"))?;
        let theme = Self::parse_theme(&content)?;

        Self::validate_theme(&theme)?;

        if let Err(e) = fs::create_dir_all(&self.themes_dir) {
            return Err(format!("Failed to create themes directory: {e}"));
        }

        let dest_path = self.themes_dir.join(format!("{}.{}", theme.id, THEME_INSTALL_EXTENSION));
        fs::write(&dest_path, content)
            .map_err(|e| format!("Failed to copy theme file: {e}"))?;

        Ok(ThemeSummary {
            id: theme.id,
            name: theme.name,
            mode: theme.mode,
            source: "custom".to_string(),
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
                if event.is_err() {
                    continue;
                }

                // Debounce: only emit theme updates at most once per second
                let now = std::time::Instant::now();
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
                    themes.push(ThemeSummary {
                        id: theme.id,
                        name: theme.name,
                        mode: theme.mode,
                        source: source.to_string(),
                    });
                }
                Err(error) => {
                    log::warn!("Invalid theme file {:?}: {error}", path.file_name());
                }
            }
        }
    }

    fn find_theme_in_dir(&self, theme_id: &str, dir: &Path) -> Option<ThemeFile> {
        if !dir.exists() {
            log::debug!("find_theme_in_dir: directory does not exist: {:?}", dir);
            return None;
        }

        let paths = theme_paths_from_dir(dir);
        log::debug!(
            "find_theme_in_dir: searching {} files in {:?} for theme '{}'",
            paths.len(),
            dir,
            theme_id
        );

        for path in paths {
            if should_skip_theme_file(&path) {
                continue;
            }
            match Self::load_theme_file(&path) {
                Ok(theme) => {
                    log::debug!(
                        "find_theme_in_dir: loaded {:?} -> id='{}'",
                        path.file_name(),
                        theme.id
                    );
                    if theme.id == theme_id {
                        log::info!(
                            "find_theme_in_dir: FOUND theme '{}' at {:?}",
                            theme_id,
                            path
                        );
                        return Some(theme);
                    }
                }
                Err(e) => {
                    log::debug!("find_theme_in_dir: failed to load {:?}: {}", path, e);
                }
            }
        }

        log::debug!(
            "find_theme_in_dir: theme '{}' NOT found in {:?}",
            theme_id,
            dir
        );
        None
    }

    fn load_theme_file(path: &Path) -> Result<ThemeFile, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read theme: {e}"))?;
        let theme = Self::parse_theme(&content)?;
        Self::validate_theme(&theme)?;
        Ok(theme)
    }

    fn find_theme_by_id(&self, theme_id: &str) -> Result<ThemeFile, String> {
        if let Some(theme) = self.find_theme_in_dir(theme_id, &self.themes_dir) {
            return Ok(theme);
        }

        if let Some(theme) = self.find_theme_in_dir(theme_id, &self.project_themes_dir) {
            return Ok(theme);
        }

        Err(format!("Theme '{theme_id}' not found"))
    }

    fn parse_theme(content: &str) -> Result<ThemeFile, String> {
        let sanitized = strip_jsonc_comments(content);
        serde_json::from_str(&sanitized)
            .map_err(|e| format!("Invalid theme JSON: {e}"))
    }

    fn validate_theme(theme: &ThemeFile) -> Result<(), String> {
        if theme.id.trim().is_empty() {
            return Err("Theme id is required".to_string());
        }
        if theme.name.trim().is_empty() {
            return Err("Theme name is required".to_string());
        }

        let id_regex = THEME_ID_REGEX.get_or_init(|| Regex::new(THEME_ID_PATTERN).unwrap());
        if !id_regex.is_match(&theme.id) {
            return Err("Theme id must be lowercase alphanumeric with dashes or underscores".to_string());
        }

        if theme.tokens.is_empty() {
            return Err("Theme tokens cannot be empty".to_string());
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
    ) -> Result<(), String> {
        let missing: Vec<&String> = required.iter().filter(|key| !tokens.contains_key(*key)).collect();
        if !missing.is_empty() {
            let preview = missing.iter().take(5).map(|k| (*k).as_str()).collect::<Vec<_>>();
            let remaining = missing.len().saturating_sub(preview.len());
            let suffix = if remaining > 0 {
                format!(" (and {remaining} more)")
            } else {
                "".to_string()
            };

            return Err(format!(
                "Missing {label} tokens: {}{suffix}",
                preview.join(", ")
            ));
        }

        for (key, value) in tokens {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(format!("Invalid {label} token '{key}': value cannot be empty"));
            }
            if trimmed.contains("REPLACE_ME") {
                return Err(format!(
                    "Invalid {label} token '{key}': replace REPLACE_ME placeholders"
                ));
            }
            if trimmed.contains("</style>") || trimmed.contains("<script") {
                return Err(format!(
                    "Invalid {label} token '{key}': value contains dangerous content"
                ));
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

fn strip_jsonc_comments(input: &str) -> String {
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
