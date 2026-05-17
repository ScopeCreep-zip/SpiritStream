use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// Theme mode: light or dark
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub enum ThemeMode {
    Light,
    Dark,
}

impl ThemeMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
        }
    }
}

// Theme summary for display in UI
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct ThemeSummary {
    pub id: String,
    pub name: String,
    pub mode: ThemeMode,
    pub source: String,
    /// True for bundled / built-in themes (UI shows badge, blocks deletion).
    pub built_in: bool,
    /// `false` when the theme file is on disk but failed to parse. The
    /// UI renders broken themes greyed-out with `error` displayed
    /// inline, instead of the previous behaviour of silently omitting
    /// them (which made operators think their edits had vanished).
    pub valid: bool,
    /// Parse / validation error message when `valid == false`.
    #[serde(default)]
    pub error: Option<String>,
}

// Single-mode theme file (NEW format)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct ThemeFile {
    pub id: String,
    pub name: String,
    pub mode: ThemeMode,                 // NEW: explicit mode declaration
    pub tokens: HashMap<String, String>, // Flat structure
}

// Legacy dual-mode theme structures removed; themes are single-mode only now.
