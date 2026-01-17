use std::collections::HashMap;
use std::path::PathBuf;

use tauri::{AppHandle, State};

use crate::models::ThemeSummary;
use crate::services::ThemeManager;

#[tauri::command]
pub fn list_themes(
    theme_manager: State<ThemeManager>,
) -> Result<Vec<ThemeSummary>, String> {
    Ok(theme_manager.list_themes())
}

#[tauri::command]
pub fn refresh_themes(
    app_handle: AppHandle,
    theme_manager: State<ThemeManager>,
) -> Result<Vec<ThemeSummary>, String> {
    // Force sync project themes and return updated list
    let _ = app_handle;
    theme_manager.sync_project_themes();
    Ok(theme_manager.list_themes())
}

#[tauri::command]
pub fn get_theme_tokens(
    theme_id: String,
    theme_manager: State<ThemeManager>,
) -> Result<HashMap<String, String>, String> {
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
