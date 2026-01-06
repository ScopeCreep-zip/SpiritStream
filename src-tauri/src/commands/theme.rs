use std::path::PathBuf;

use tauri::{AppHandle, State};

use crate::models::{ThemeSummary, ThemeTokens};
use crate::services::ThemeManager;

#[tauri::command]
pub fn list_themes(
    app_handle: AppHandle,
    theme_manager: State<ThemeManager>,
) -> Result<Vec<ThemeSummary>, String> {
    Ok(theme_manager.list_themes(Some(&app_handle)))
}

#[tauri::command]
pub fn get_theme_tokens(
    theme_id: String,
    theme_manager: State<ThemeManager>,
) -> Result<ThemeTokens, String> {
    theme_manager.get_theme_tokens(&theme_id)
}

#[tauri::command]
pub fn install_theme(
    theme_path: String,
    theme_manager: State<ThemeManager>,
) -> Result<ThemeSummary, String> {
    let path = PathBuf::from(theme_path);
    theme_manager.install_theme(&path)
}
