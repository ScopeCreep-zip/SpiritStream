// Settings Commands
// Tauri command handlers for settings management

use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};
use crate::models::Settings;
use crate::services::{prune_logs, SettingsManager, Encryption, RotationReport};

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
    app_handle: AppHandle,
) -> Result<(), String> {
    settings_manager.save(&settings)?;
    if let Ok(log_dir) = app_handle.path().app_log_dir() {
        let _ = prune_logs(&log_dir, settings.log_retention_days);
    }
    Ok(())
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

/// Rotate the machine encryption key
/// Re-encrypts all stream keys in all profiles with a new machine key
#[tauri::command]
pub fn rotate_machine_key(app_handle: AppHandle) -> Result<RotationReport, String> {
    log::info!("Machine key rotation requested");

    // Get app data directory
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

    // Get profiles directory
    let profiles_dir = app_data_dir.join("profiles");

    // Perform rotation
    Encryption::rotate_machine_key(&app_data_dir, &profiles_dir)
}
