use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use regex::Regex;
use tauri::{AppHandle, Emitter, Manager};

use crate::models::{ThemeFile, ThemeSummary, ThemeTokens};

const BUILTIN_THEME_ID: &str = "spirit";
const THEME_FILE_EXTENSIONS: [&str; 2] = ["json", "jsonc"];
const THEME_INSTALL_EXTENSION: &str = "jsonc";
const THEME_ID_PATTERN: &str = r"^[a-z0-9][a-z0-9-_]{0,63}$";
const TOKENS_CSS: &str = include_str!("../../../src-frontend/styles/tokens.css");

static REQUIRED_TOKENS: OnceLock<Vec<String>> = OnceLock::new();
static THEME_ID_REGEX: OnceLock<Regex> = OnceLock::new();

#[derive(Clone)]
pub struct ThemeManager {
    themes_dir: PathBuf,
}

impl ThemeManager {
    /// Syncs built-in/project themes to the appdata themes directory, skipping theme-template.jsonc
    ///
    /// In production: Uses Tauri's resource_dir to access bundled themes
    /// In development: Falls back to ../themes relative path
    pub fn sync_project_themes(&self, app_handle: Option<&AppHandle>) {
        // Try production path first (bundled resources)
        let project_themes_dir = if let Some(handle) = app_handle {
            if let Ok(resource_dir) = handle.path().resource_dir() {
                let bundled_themes = resource_dir.join("themes");
                if bundled_themes.exists() {
                    log::info!("Using bundled themes from: {:?}", bundled_themes);
                    bundled_themes
                } else {
                    // Fallback to dev path
                    log::info!("Bundled themes not found, using dev path");
                    PathBuf::from("../themes")
                }
            } else {
                // Fallback to dev path
                PathBuf::from("../themes")
            }
        } else {
            // No app handle provided, use dev path
            PathBuf::from("../themes")
        };

        let absolute_project_themes = if project_themes_dir.is_absolute() {
            project_themes_dir.clone()
        } else {
            std::env::current_dir()
                .ok()
                .map(|d| d.join(&project_themes_dir))
                .unwrap_or_else(|| project_themes_dir.clone())
        };

        log::info!("Syncing themes from {:?} to {:?}", absolute_project_themes, self.themes_dir);

        match fs::read_dir(&project_themes_dir) {
            Ok(entries) => {
                let mut copied = 0;
                for entry in entries.flatten() {
                    let path = entry.path();
                    // Only .jsonc or .json files, skip template
                    let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if fname == "theme-template.jsonc" {
                        log::debug!("Skipping theme template");
                        continue;
                    }
                    if !is_theme_file(&path) {
                        log::debug!("Skipping non-theme file: {:?}", fname);
                        continue;
                    }
                    // Copy if not present or if updated
                    let dest_path = self.themes_dir.join(fname);
                    let should_copy = match (fs::metadata(&path), fs::metadata(&dest_path)) {
                        (Ok(src), Ok(dst)) => src.modified().ok() > dst.modified().ok(),
                        (Ok(_), Err(_)) => true,
                        _ => false,
                    };
                    if should_copy {
                        match fs::copy(&path, &dest_path) {
                            Ok(_) => {
                                log::info!("Copied theme {:?} to appdata", fname);
                                copied += 1;
                            }
                            Err(e) => {
                                log::error!("Failed to copy theme {:?} to appdata: {e}", fname);
                            }
                        }
                    }
                }
                log::info!("Theme sync complete: {copied} theme(s) copied");
            }
            Err(e) => {
                log::error!("Failed to read project themes directory {:?}: {e}", absolute_project_themes);
            }
        }
    }
    pub fn new(app_data_dir: PathBuf) -> Self {
        let themes_dir = app_data_dir.join("themes");
        if let Err(e) = fs::create_dir_all(&themes_dir) {
            log::warn!("Failed to create themes directory: {e}");
        }
        Self { themes_dir }
    }

    pub fn list_themes(&self, app_handle: Option<&AppHandle>) -> Vec<ThemeSummary> {
        // Always sync project themes before listing
        self.sync_project_themes(app_handle);
        let mut themes = Vec::new();
        let mut seen_ids = HashSet::new();
        themes.push(ThemeSummary {
            id: BUILTIN_THEME_ID.to_string(),
            name: "Spirit".to_string(),
            source: "builtin".to_string(),
        });
        seen_ids.insert(BUILTIN_THEME_ID.to_string());

        if let Ok(entries) = fs::read_dir(&self.themes_dir) {
            let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
            paths.sort_by_key(|path| {
                match path.extension().and_then(|s| s.to_str()) {
                    Some("jsonc") => 0,
                    Some("json") => 1,
                    _ => 2,
                }
            });

            for path in paths {
                if !is_theme_file(&path) {
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
                            source: "custom".to_string(),
                        });
                    }
                    Err(error) => {
                        log::warn!("Invalid theme file {:?}: {error}", path.file_name());
                    }
                }
            }
        }

        themes
    }

    pub fn get_theme_tokens(&self, theme_id: &str) -> Result<ThemeTokens, String> {
        if theme_id == BUILTIN_THEME_ID {
            return Err("Built-in theme does not provide tokens".to_string());
        }

        let theme = self.find_theme_by_id(theme_id)?;
        Ok(theme.tokens)
    }

    pub fn install_theme(&self, source_path: &Path) -> Result<ThemeSummary, String> {
        let content = fs::read_to_string(source_path)
            .map_err(|e| format!("Failed to read theme file: {e}"))?;
        let theme = Self::parse_theme(&content)?;

        Self::validate_theme(&theme)?;

        if theme.id == BUILTIN_THEME_ID {
            return Err("Theme id 'spirit' is reserved".to_string());
        }

        if let Err(e) = fs::create_dir_all(&self.themes_dir) {
            return Err(format!("Failed to create themes directory: {e}"));
        }

        let dest_path = self.themes_dir.join(format!("{}.{}", theme.id, THEME_INSTALL_EXTENSION));
        fs::write(&dest_path, content)
            .map_err(|e| format!("Failed to copy theme file: {e}"))?;

        Ok(ThemeSummary {
            id: theme.id,
            name: theme.name,
            source: "custom".to_string(),
        })
    }

    pub fn start_watcher(&self, app_handle: AppHandle) {
        let manager = self.clone();
        let themes_dir = self.themes_dir.clone();
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

            for event in rx {
                if event.is_err() {
                    continue;
                }

                let themes = manager.list_themes(None);
                let _ = app_handle.emit("themes_updated", themes);
                thread::sleep(Duration::from_millis(50));
            }
        });
    }

    fn load_theme_file(path: &Path) -> Result<ThemeFile, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read theme: {e}"))?;
        let theme = Self::parse_theme(&content)?;
        Self::validate_theme(&theme)?;
        Ok(theme)
    }

    fn find_theme_by_id(&self, theme_id: &str) -> Result<ThemeFile, String> {
        let entries = fs::read_dir(&self.themes_dir)
            .map_err(|e| format!("Failed to read themes directory: {e}"))?;

        let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
        paths.sort_by_key(|path| {
            match path.extension().and_then(|s| s.to_str()) {
                Some("jsonc") => 0,
                Some("json") => 1,
                _ => 2,
            }
        });

        for path in paths {
            if !is_theme_file(&path) {
                continue;
            }
            if let Ok(theme) = Self::load_theme_file(&path) {
                if theme.id == theme_id {
                    return Ok(theme);
                }
            }
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

        if theme.tokens.light.is_empty() || theme.tokens.dark.is_empty() {
            return Err("Theme tokens must include light and dark variants".to_string());
        }

        let required = required_tokens();
        Self::validate_token_set("light", &theme.tokens.light, required)?;
        Self::validate_token_set("dark", &theme.tokens.dark, required)?;

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
        for cap in token_regex.captures_iter(TOKENS_CSS) {
            if let Some(matched) = cap.get(0) {
                tokens.insert(matched.as_str().to_string());
            }
        }
        let mut tokens: Vec<String> = tokens.into_iter().collect();
        tokens.sort();
        tokens
    })
}

fn is_theme_file(path: &Path) -> bool {
    match path.extension().and_then(|s| s.to_str()) {
        Some(ext) => THEME_FILE_EXTENSIONS.iter().any(|allowed| *allowed == ext),
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
