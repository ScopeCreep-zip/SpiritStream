// Profile Commands
// Handles profile CRUD operations

use crate::models::Profile;

/// Get all profile names from the profiles directory
#[tauri::command]
pub async fn get_all_profiles() -> Result<Vec<String>, String> {
    // TODO: Implement with ProfileManager service
    Ok(vec![])
}

/// Load a profile by name
#[tauri::command]
pub async fn load_profile(name: String, password: Option<String>) -> Result<Profile, String> {
    // TODO: Implement with ProfileManager service
    Err("Not implemented".to_string())
}

/// Save a profile
#[tauri::command]
pub async fn save_profile(profile: Profile, password: Option<String>) -> Result<(), String> {
    // TODO: Implement with ProfileManager service
    Err("Not implemented".to_string())
}

/// Delete a profile by name
#[tauri::command]
pub async fn delete_profile(name: String) -> Result<(), String> {
    // TODO: Implement with ProfileManager service
    Err("Not implemented".to_string())
}
