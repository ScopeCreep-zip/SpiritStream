// ProfileManager Service
// Handles profile persistence and encryption

use std::path::PathBuf;
use crate::models::Profile;

/// Manages profile storage and retrieval
pub struct ProfileManager {
    profiles_dir: PathBuf,
}

impl ProfileManager {
    /// Create a new ProfileManager with the given app data directory
    pub fn new(app_data_dir: PathBuf) -> Self {
        let profiles_dir = app_data_dir.join("profiles");
        std::fs::create_dir_all(&profiles_dir).ok();
        Self { profiles_dir }
    }

    /// Get all profile names from the profiles directory
    pub async fn get_all_names(&self) -> Result<Vec<String>, String> {
        let mut names = Vec::new();

        let entries = std::fs::read_dir(&self.profiles_dir)
            .map_err(|e| e.to_string())?;

        for entry in entries {
            if let Ok(entry) = entry {
                if let Some(name) = entry.path().file_stem() {
                    if entry.path().extension().map_or(false, |ext| ext == "json") {
                        names.push(name.to_string_lossy().to_string());
                    }
                }
            }
        }

        Ok(names)
    }

    /// Load a profile by name
    pub async fn load(&self, name: &str, _password: Option<&str>) -> Result<Profile, String> {
        let path = self.profiles_dir.join(format!("{}.json", name));
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read profile: {}", e))?;

        // TODO: Decrypt if password provided

        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse profile: {}", e))
    }

    /// Save a profile
    pub async fn save(&self, profile: &Profile, _password: Option<&str>) -> Result<(), String> {
        let path = self.profiles_dir.join(format!("{}.json", profile.name));

        // TODO: Encrypt if password provided

        let content = serde_json::to_string_pretty(profile)
            .map_err(|e| format!("Failed to serialize profile: {}", e))?;

        std::fs::write(&path, content)
            .map_err(|e| format!("Failed to write profile: {}", e))
    }

    /// Delete a profile by name
    pub async fn delete(&self, name: &str) -> Result<(), String> {
        let path = self.profiles_dir.join(format!("{}.json", name));
        std::fs::remove_file(&path)
            .map_err(|e| format!("Failed to delete profile: {}", e))
    }
}
