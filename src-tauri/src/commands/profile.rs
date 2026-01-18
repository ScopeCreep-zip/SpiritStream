// Profile Commands
// Handles profile CRUD operations

use tauri::State;
use crate::models::{Profile, ProfileSummary, RtmpInput, OrderIndexMap};
use crate::services::{ProfileManager, SettingsManager};


/// Persist profile order based on UI order (names in desired order)
#[tauri::command]
pub async fn set_profile_order(
    ordered_names: Vec<String>,
    profile_manager: State<'_, ProfileManager>
) -> Result<(), String> { 
    let mut map = profile_manager.read_order_index_map()?;
    let existing = profile_manager.get_all_names().await?; 
    

    let mut idx = 0;
    for name in ordered_names {
       if !existing.contains(&name){
            return Err(format!("Unknown profile: {}", name));
        } 
        idx += 10;
        map.insert(name, idx);
        
    }

    profile_manager.write_order_index_map(&map)?;
    Ok(())
}


/// Get the order index map (name -> order_index)
#[tauri::command]
pub fn get_order_index_map(
    profile_manager: State<'_, ProfileManager>
) -> Result<OrderIndexMap, String> {
    profile_manager.read_order_index_map()
}

/// Get all profile names from the profiles directory
#[tauri::command]
pub async fn get_all_profiles(
    profile_manager: State<'_, ProfileManager>
) -> Result<Vec<String>, String> {
    profile_manager.get_all_names().await
}

/// Get summaries of all profiles for list display
/// Includes services list for showing platform icons (Story 1.1, 4.1, 4.2)
#[tauri::command]
pub async fn get_profile_summaries(
    profile_manager: State<'_, ProfileManager>
) -> Result<Vec<ProfileSummary>, String> {
    profile_manager.get_all_summaries().await
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

/// Validate that an RTMP input configuration doesn't conflict with existing profiles
/// Used to prevent port conflicts in shared studio environments (Story 2.2)
#[tauri::command]
pub async fn validate_input(
    profile_id: String,
    input: RtmpInput,
    profile_manager: State<'_, ProfileManager>,
) -> Result<(), String> {
    profile_manager.validate_input_conflict(&profile_id, &input).await
}
