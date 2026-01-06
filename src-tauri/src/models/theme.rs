use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// Theme mode: light or dark
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSummary {
    pub id: String,
    pub name: String,
    pub mode: ThemeMode,  // NEW: explicit mode
    pub source: String,
}

// Single-mode theme file (NEW format)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeFile {
    pub id: String,
    pub name: String,
    pub mode: ThemeMode,  // NEW: explicit mode declaration
    pub tokens: HashMap<String, String>,  // Flat structure
}

// DEPRECATED: Dual-mode theme structure for backward compatibility
// Will be removed in v0.3.0
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyThemeFile {
    pub id: String,
    pub name: String,
    pub tokens: LegacyThemeTokens,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyThemeTokens {
    pub light: HashMap<String, String>,
    pub dark: HashMap<String, String>,
}

// DEPRECATED: Old ThemeTokens for frontend compatibility during transition
// Frontend will be updated to use flat HashMap directly
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeTokens {
    pub light: HashMap<String, String>,
    pub dark: HashMap<String, String>,
}
