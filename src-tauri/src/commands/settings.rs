// Settings Commands
// Tauri command handlers for settings management

use std::path::PathBuf;
use tauri::State;
use crate::models::Settings;
use crate::services::SettingsManager;

/// Get current settings
#[tauri::command]
pub fn get_settings(settings_manager: State<SettingsManager>) -> Result<Settings, String> {
    settings_manager.load()
}

/// Save settings
#[tauri::command]
pub fn save_settings(
    settings: Settings,
    settings_manager: State<SettingsManager>,
) -> Result<(), String> {
    settings_manager.save(&settings)
}

/// Get profiles storage path
#[tauri::command]
pub fn get_profiles_path(settings_manager: State<SettingsManager>) -> String {
    settings_manager.get_profiles_path().to_string_lossy().to_string()
}

/// Export all app data to specified path
#[tauri::command]
pub fn export_data(
    export_path: String,
    settings_manager: State<SettingsManager>,
) -> Result<(), String> {
    let path = PathBuf::from(export_path);
    settings_manager.export_data(&path)
}

/// Clear all app data (destructive)
#[tauri::command]
pub fn clear_data(settings_manager: State<SettingsManager>) -> Result<(), String> {
    settings_manager.clear_data()
}
