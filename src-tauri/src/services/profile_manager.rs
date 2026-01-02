// ProfileManager Service
// Handles profile persistence and encryption

use std::path::PathBuf;
use crate::models::Profile;
use crate::services::Encryption;

// Magic bytes to identify encrypted profiles
const ENCRYPTED_MAGIC: &[u8] = b"MGLA";

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
                let path = entry.path();
                if let Some(name) = path.file_stem() {
                    // Accept both .json (unencrypted) and .mgs (encrypted) files
                    let ext = path.extension().and_then(|e| e.to_str());
                    if ext == Some("json") || ext == Some("mgs") {
                        names.push(name.to_string_lossy().to_string());
                    }
                }
            }
        }

        Ok(names)
    }

    /// Load a profile by name
    /// If password is provided, will attempt to decrypt
    /// If no password, will try unencrypted first, then fail if encrypted
    pub async fn load(&self, name: &str, password: Option<&str>) -> Result<Profile, String> {
        // Try encrypted file first (if password provided)
        let encrypted_path = self.profiles_dir.join(format!("{}.mgs", name));
        let json_path = self.profiles_dir.join(format!("{}.json", name));

        // Check if encrypted version exists
        if encrypted_path.exists() {
            let data = std::fs::read(&encrypted_path)
                .map_err(|e| format!("Failed to read profile: {}", e))?;

            // Verify magic bytes
            if data.len() < ENCRYPTED_MAGIC.len() || &data[..ENCRYPTED_MAGIC.len()] != ENCRYPTED_MAGIC {
                return Err("Invalid encrypted profile format".to_string());
            }

            // Password required for encrypted profiles
            let password = password.ok_or("Password required for encrypted profile")?;

            // Decrypt (skip magic bytes)
            let decrypted = Encryption::decrypt(&data[ENCRYPTED_MAGIC.len()..], password)?;

            // Parse JSON
            let content = String::from_utf8(decrypted)
                .map_err(|e| format!("Invalid UTF-8 in decrypted profile: {}", e))?;

            return serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse profile: {}", e));
        }

        // Try unencrypted JSON file
        if json_path.exists() {
            let content = std::fs::read_to_string(&json_path)
                .map_err(|e| format!("Failed to read profile: {}", e))?;

            return serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse profile: {}", e));
        }

        Err(format!("Profile '{}' not found", name))
    }

    /// Save a profile
    /// If password is provided, will encrypt the profile
    pub async fn save(&self, profile: &Profile, password: Option<&str>) -> Result<(), String> {
        // Serialize to JSON
        let content = serde_json::to_string_pretty(profile)
            .map_err(|e| format!("Failed to serialize profile: {}", e))?;

        if let Some(pwd) = password {
            // Encrypt and save as .mgs file
            let encrypted = Encryption::encrypt(content.as_bytes(), pwd)?;

            // Prepend magic bytes
            let mut data = Vec::with_capacity(ENCRYPTED_MAGIC.len() + encrypted.len());
            data.extend_from_slice(ENCRYPTED_MAGIC);
            data.extend_from_slice(&encrypted);

            let path = self.profiles_dir.join(format!("{}.mgs", profile.name));
            std::fs::write(&path, data)
                .map_err(|e| format!("Failed to write encrypted profile: {}", e))?;

            // Remove unencrypted version if it exists
            let json_path = self.profiles_dir.join(format!("{}.json", profile.name));
            if json_path.exists() {
                std::fs::remove_file(&json_path).ok();
            }
        } else {
            // Save as unencrypted JSON
            let path = self.profiles_dir.join(format!("{}.json", profile.name));
            std::fs::write(&path, content)
                .map_err(|e| format!("Failed to write profile: {}", e))?;

            // Remove encrypted version if it exists
            let mgs_path = self.profiles_dir.join(format!("{}.mgs", profile.name));
            if mgs_path.exists() {
                std::fs::remove_file(&mgs_path).ok();
            }
        }

        Ok(())
    }

    /// Delete a profile by name (both encrypted and unencrypted versions)
    pub async fn delete(&self, name: &str) -> Result<(), String> {
        let json_path = self.profiles_dir.join(format!("{}.json", name));
        let mgs_path = self.profiles_dir.join(format!("{}.mgs", name));

        let mut deleted = false;

        if json_path.exists() {
            std::fs::remove_file(&json_path)
                .map_err(|e| format!("Failed to delete profile: {}", e))?;
            deleted = true;
        }

        if mgs_path.exists() {
            std::fs::remove_file(&mgs_path)
                .map_err(|e| format!("Failed to delete encrypted profile: {}", e))?;
            deleted = true;
        }

        if deleted {
            Ok(())
        } else {
            Err(format!("Profile '{}' not found", name))
        }
    }

    /// Check if a profile is encrypted
    pub fn is_encrypted(&self, name: &str) -> bool {
        let mgs_path = self.profiles_dir.join(format!("{}.mgs", name));
        mgs_path.exists()
    }
}
