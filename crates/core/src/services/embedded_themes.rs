use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::models::{ThemeMode, ThemeSummary};

// Embed all theme files at compile time
const SPIRIT_LIGHT_JSON: &str = include_str!("../../../../themes/spirit-light.jsonc");
const SPIRIT_DARK_JSON: &str = include_str!("../../../../themes/spirit-dark.jsonc");
const CATPPUCCIN_MOCHA_DARK_JSON: &str =
    include_str!("../../../../themes/catppuccin-mocha-dark.jsonc");
const CATPPUCCIN_MOCHA_LIGHT_JSON: &str =
    include_str!("../../../../themes/catppuccin-mocha-light.jsonc");
const DRACULA_JSON: &str = include_str!("../../../../themes/dracula.jsonc");
const KALIFORNIA_JSON: &str = include_str!("../../../../themes/kalifornia.jsonc");
const NORD_DARK_JSON: &str = include_str!("../../../../themes/nord-dark.jsonc");
const NORD_LIGHT_JSON: &str = include_str!("../../../../themes/nord-light.jsonc");
const RAINBOW_PRIDE_DARK_JSON: &str = include_str!("../../../../themes/rainbow-pride-dark.jsonc");
const RAINBOW_PRIDE_LIGHT_JSON: &str = include_str!("../../../../themes/rainbow-pride-light.jsonc");
const TRANS_PRIDE_DARK_JSON: &str = include_str!("../../../../themes/trans-pride-dark.jsonc");
const TRANS_PRIDE_LIGHT_JSON: &str = include_str!("../../../../themes/trans-pride-light.jsonc");
// High-contrast variants meeting WCAG AAA 7:1. Embedded as a
// defense-in-depth fallback for the disabled / low-vision population:
// if the bundled `themes/*.jsonc` filesystem copy ever fails at install
// or first launch (permission denied, disk full, sandbox restriction),
// these tokens remain reachable so accessibility users keep their theme.
const HIGH_CONTRAST_DARK_JSON: &str = include_str!("../../../../themes/high-contrast-dark.jsonc");
const HIGH_CONTRAST_LIGHT_JSON: &str = include_str!("../../../../themes/high-contrast-light.jsonc");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThemeJson {
    id: String,
    name: String,
    mode: ThemeMode,
    tokens: HashMap<String, String>,
}

static EMBEDDED_THEMES: OnceLock<HashMap<String, HashMap<String, String>>> = OnceLock::new();
static EMBEDDED_THEME_LIST: OnceLock<Vec<ThemeSummary>> = OnceLock::new();

/// Get tokens for an embedded theme by ID
/// Returns None if theme is not embedded (user-installed custom theme)
pub fn get_embedded_theme_tokens(theme_id: &str) -> Option<HashMap<String, String>> {
    let themes = EMBEDDED_THEMES.get_or_init(|| {
        let mut map = HashMap::new();

        // List of all embedded theme JSON strings
        let theme_sources = [
            SPIRIT_LIGHT_JSON,
            SPIRIT_DARK_JSON,
            CATPPUCCIN_MOCHA_DARK_JSON,
            CATPPUCCIN_MOCHA_LIGHT_JSON,
            DRACULA_JSON,
            KALIFORNIA_JSON,
            NORD_DARK_JSON,
            NORD_LIGHT_JSON,
            RAINBOW_PRIDE_DARK_JSON,
            RAINBOW_PRIDE_LIGHT_JSON,
            TRANS_PRIDE_DARK_JSON,
            TRANS_PRIDE_LIGHT_JSON,
            HIGH_CONTRAST_DARK_JSON,
            HIGH_CONTRAST_LIGHT_JSON,
        ];

        for json_str in theme_sources {
            // Strip JSONC comments + trailing commas before parsing.
            let clean_json = crate::services::jsonc::sanitize_jsonc(json_str);
            match serde_json::from_str::<ThemeJson>(&clean_json) {
                Ok(theme) => {
                    log::info!(
                        "Embedded theme '{}' with {} tokens",
                        theme.id,
                        theme.tokens.len()
                    );
                    map.insert(theme.id, theme.tokens);
                }
                Err(e) => {
                    log::warn!("Failed to parse embedded theme JSON: {e}");
                }
            }
        }

        log::info!("Loaded {} embedded themes", map.len());
        map
    });

    themes.get(theme_id).cloned()
}

/// Check if a theme ID has embedded tokens.
/// Used by `ThemeManager` to populate `ThemeSummary.built_in` so the UI
/// can render a "Built-in" badge and gate deletion.
pub fn is_embedded_theme(theme_id: &str) -> bool {
    get_embedded_theme_tokens(theme_id).is_some()
}

/// Get list of all embedded themes as ThemeSummary
/// Used as fallback when theme files are not accessible (production builds)
pub fn get_embedded_theme_list() -> Vec<ThemeSummary> {
    let list = EMBEDDED_THEME_LIST.get_or_init(|| {
        let theme_sources = [
            SPIRIT_LIGHT_JSON,
            SPIRIT_DARK_JSON,
            CATPPUCCIN_MOCHA_DARK_JSON,
            CATPPUCCIN_MOCHA_LIGHT_JSON,
            DRACULA_JSON,
            KALIFORNIA_JSON,
            NORD_DARK_JSON,
            NORD_LIGHT_JSON,
            RAINBOW_PRIDE_DARK_JSON,
            RAINBOW_PRIDE_LIGHT_JSON,
            TRANS_PRIDE_DARK_JSON,
            TRANS_PRIDE_LIGHT_JSON,
            HIGH_CONTRAST_DARK_JSON,
            HIGH_CONTRAST_LIGHT_JSON,
        ];

        let mut summaries = Vec::new();
        for json_str in theme_sources {
            let clean_json = crate::services::jsonc::sanitize_jsonc(json_str);
            match serde_json::from_str::<ThemeJson>(&clean_json) {
                Ok(theme) => {
                    summaries.push(ThemeSummary {
                        id: theme.id,
                        name: theme.name,
                        mode: theme.mode,
                        source: "builtin".to_string(),
                        built_in: true,
                        valid: true,
                        error: None,
                    });
                }
                Err(e) => {
                    log::warn!("Failed to parse embedded theme for list: {e}");
                }
            }
        }

        log::info!("Built embedded theme list with {} themes", summaries.len());
        summaries
    });

    list.clone()
}
