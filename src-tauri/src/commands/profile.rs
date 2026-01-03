// Profile Commands
// Handles profile CRUD operations

use tauri::State;
use crate::models::Profile;
use crate::services::ProfileManager;

/// Get all profile names from the profiles directory
#[tauri::command]
pub async fn get_all_profiles(
    profile_manager: State<'_, ProfileManager>
) -> Result<Vec<String>, String> {
    profile_manager.get_all_names().await
}

/// Load a profile by name
#[tauri::command]
pub async fn load_profile(
    name: String,
    password: Option<String>,
    profile_manager: State<'_, ProfileManager>
) -> Result<Profile, String> {
    profile_manager.load(&name, password.as_deref()).await
}

/// Save a profile
#[tauri::command]
pub async fn save_profile(
    profile: Profile,
    password: Option<String>,
    profile_manager: State<'_, ProfileManager>
) -> Result<(), String> {
    profile_manager.save(&profile, password.as_deref()).await
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
