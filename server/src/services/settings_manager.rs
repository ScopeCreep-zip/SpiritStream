// SettingsManager Service
// Handles application settings persistence

use std::path::PathBuf;
use std::sync::RwLock;
use crate::models::Settings;
use crate::services::encryption::Encryption;
use serde_json::Value;

/// Fields in settings.json (camelCase) that contain sensitive data and should be encrypted at rest
const SENSITIVE_FIELDS: &[&str] = &[
    "twitchOauthAccessToken",
    "twitchOauthRefreshToken",
    "youtubeOauthAccessToken",
    "youtubeOauthRefreshToken",
    "chatYoutubeApiKey",
    "obsPassword",
    "backendToken",
    "discordWebhookUrl",
];

/// Manages application settings storage and retrieval
pub struct SettingsManager {
    settings_path: PathBuf,
    app_data_dir: PathBuf,
    cache: RwLock<Option<Settings>>,
}

impl SettingsManager {
    /// Create a new SettingsManager with the given app data directory
    pub fn new(app_data_dir: PathBuf) -> Self {
        let settings_path = app_data_dir.join("settings.json");
        Self {
            settings_path,
            app_data_dir,
            cache: RwLock::new(None),
        }
    }

    /// Load settings from disk, or return defaults if not found
    pub fn load(&self) -> Result<Settings, String> {
        // Check cache first
        if let Ok(cache) = self.cache.read() {
            if let Some(ref settings) = *cache {
                return Ok(settings.clone());
            }
        }

        // Try to read from disk
        let settings = if self.settings_path.exists() {
            let content = std::fs::read_to_string(&self.settings_path)
                .map_err(|e| format!("Failed to read settings: {e}"))?;

            let mut user_value: Value = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse settings: {e}"))?;
            let defaults_value = serde_json::to_value(Settings::default())
                .map_err(|e| format!("Failed to build default settings: {e}"))?;

            let changed = merge_missing_settings(&mut user_value, &defaults_value);

            // Decrypt sensitive fields (ENC:: values become plaintext in memory)
            self.decrypt_sensitive_fields(&mut user_value);

            let settings: Settings = serde_json::from_value(user_value)
                .map_err(|e| format!("Failed to parse settings: {e}"))?;

            if changed {
                self.save_internal(&settings)?;
            }

            settings
        } else {
            // Return defaults and save them
            let defaults = Settings::default();
            self.save_internal(&defaults)?;
            defaults
        };

        // Update cache
        if let Ok(mut cache) = self.cache.write() {
            *cache = Some(settings.clone());
        }

        Ok(settings)
    }

    /// Save settings to disk
    pub fn save(&self, settings: &Settings) -> Result<(), String> {
        self.save_internal(settings)?;

        // Update cache
        if let Ok(mut cache) = self.cache.write() {
            *cache = Some(settings.clone());
        }

        Ok(())
    }

    /// Internal save without cache update
    fn save_internal(&self, settings: &Settings) -> Result<(), String> {
        // Ensure parent directory exists
        if let Some(parent) = self.settings_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create settings directory: {e}"))?;
        }

        let mut value = serde_json::to_value(settings)
            .map_err(|e| format!("Failed to serialize settings: {e}"))?;

        // Encrypt sensitive fields before writing to disk
        self.encrypt_sensitive_fields(&mut value);

        let content = serde_json::to_string_pretty(&value)
            .map_err(|e| format!("Failed to serialize settings: {e}"))?;

        std::fs::write(&self.settings_path, content)
            .map_err(|e| format!("Failed to write settings: {e}"))
    }

    /// Get the path to the profiles directory
    pub fn get_profiles_path(&self) -> PathBuf {
        self.settings_path
            .parent()
            .map(|p| p.join("profiles"))
            .unwrap_or_else(|| PathBuf::from("profiles"))
    }

    /// Export all app data to a directory
    pub fn export_data(&self, export_path: &PathBuf) -> Result<(), String> {
        std::fs::create_dir_all(export_path)
            .map_err(|e| format!("Failed to create export directory: {e}"))?;

        // Export settings
        if self.settings_path.exists() {
            let dest = export_path.join("settings.json");
            std::fs::copy(&self.settings_path, &dest)
                .map_err(|e| format!("Failed to export settings: {e}"))?;
        }

        // Export profiles
        let profiles_dir = self.get_profiles_path();
        if profiles_dir.exists() {
            let export_profiles_dir = export_path.join("profiles");
            std::fs::create_dir_all(&export_profiles_dir)
                .map_err(|e| format!("Failed to create profiles export directory: {e}"))?;

            for entry in std::fs::read_dir(&profiles_dir).map_err(|e| e.to_string())?.flatten() {
                let dest = export_profiles_dir.join(entry.file_name());
                std::fs::copy(entry.path(), dest)
                    .map_err(|e| format!("Failed to export profile: {e}"))?;
            }
        }

        Ok(())
    }

    /// Decrypt sensitive fields in a JSON Value (ENC:: -> plaintext)
    /// Used after reading from disk so in-memory Settings always has plaintext
    fn decrypt_sensitive_fields(&self, value: &mut Value) {
        if let Value::Object(map) = value {
            for &field in SENSITIVE_FIELDS {
                if let Some(Value::String(val)) = map.get(field) {
                    if Encryption::is_encrypted(val) {
                        match Encryption::decrypt_token(val, &self.app_data_dir) {
                            Ok(plaintext) => {
                                map.insert(field.to_string(), Value::String(plaintext));
                            }
                            Err(e) => {
                                log::warn!("Failed to decrypt settings field '{}': {}", field, e);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Encrypt sensitive fields in a JSON Value (plaintext -> ENC::)
    /// Used before writing to disk so on-disk settings always has encrypted tokens
    fn encrypt_sensitive_fields(&self, value: &mut Value) {
        if let Value::Object(map) = value {
            for &field in SENSITIVE_FIELDS {
                if let Some(Value::String(val)) = map.get(field) {
                    if !val.is_empty() && !Encryption::is_encrypted(val) {
                        match Encryption::encrypt_token(val, &self.app_data_dir) {
                            Ok(encrypted) => {
                                map.insert(field.to_string(), Value::String(encrypted));
                            }
                            Err(e) => {
                                log::warn!("Failed to encrypt settings field '{}': {}", field, e);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Clear all app data
    pub fn clear_data(&self) -> Result<(), String> {
        // Clear settings cache
        if let Ok(mut cache) = self.cache.write() {
            *cache = None;
        }

        // Delete settings file
        if self.settings_path.exists() {
            std::fs::remove_file(&self.settings_path)
                .map_err(|e| format!("Failed to delete settings: {e}"))?;
        }

        // Delete profiles directory
        let profiles_dir = self.get_profiles_path();
        if profiles_dir.exists() {
            std::fs::remove_dir_all(&profiles_dir)
                .map_err(|e| format!("Failed to delete profiles: {e}"))?;
        }

        Ok(())
    }
}

fn merge_missing_settings(target: &mut Value, defaults: &Value) -> bool {
    match (target, defaults) {
        (Value::Object(target_map), Value::Object(defaults_map)) => {
            let mut changed = false;
            for (key, default_value) in defaults_map {
                match target_map.get_mut(key) {
                    Some(target_value) => {
                        if merge_missing_settings(target_value, default_value) {
                            changed = true;
                        }
                    }
                    None => {
                        target_map.insert(key.clone(), default_value.clone());
                        changed = true;
                    }
                }
            }
            changed
        }
        _ => false,
    }
}
