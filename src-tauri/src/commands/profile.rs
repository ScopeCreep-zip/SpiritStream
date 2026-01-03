// Profile Commands
// Handles profile CRUD operations

use tauri::State;
use crate::models::Profile;
use crate::services::{ProfileManager, SettingsManager};

/// Get all profile names from the profiles directory
#[tauri::command]
pub async fn get_all_profiles(
    profile_manager: State<'_, ProfileManager>
) -> Result<Vec<String>, String> {
    profile_manager.get_all_names().await
}

/// Load a profile by name
/// Always decrypts stream keys if they were encrypted
#[tauri::command]
pub async fn load_profile(
    name: String,
    password: Option<String>,
    profile_manager: State<'_, ProfileManager>
) -> Result<Profile, String> {
    // Use the new method that handles stream key decryption
    profile_manager.load_with_key_decryption(&name, password.as_deref()).await
}

/// Save a profile
/// Respects the encrypt_stream_keys setting from app settings
#[tauri::command]
pub async fn save_profile(
    profile: Profile,
    password: Option<String>,
    profile_manager: State<'_, ProfileManager>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    // Get the encrypt_stream_keys setting
    let settings = settings_manager.load()?;
    let encrypt_keys = settings.encrypt_stream_keys;

    // Use the new method that handles stream key encryption
    profile_manager.save_with_key_encryption(&profile, password.as_deref(), encrypt_keys).await
}

/// Delete a profile by name
#[tauri::command]
pub async fn delete_profile(
    name: String,
    profile_manager: State<'_, ProfileManager>
) -> Result<(), String> {
    profile_manager.delete(&name).await
}

/// Check if a profile is encrypted (requires password to load)
#[tauri::command]
pub fn is_profile_encrypted(
    name: String,
    profile_manager: State<'_, ProfileManager>
) -> bool {
    profile_manager.is_encrypted(&name)
}
