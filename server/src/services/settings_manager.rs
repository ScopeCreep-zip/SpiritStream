// SettingsManager Service
// Handles application settings persistence

use std::path::PathBuf;
use std::sync::RwLock;
use crate::models::{Settings, ProfileSettings, ObsIntegrationDirection};
use crate::services::encryption::Encryption;
use serde_json::Value;

/// Legacy fields in settings.json (camelCase) that contain sensitive data and should be decrypted
/// so we can migrate them into per-profile settings.
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
    legacy_profile_settings: RwLock<Option<ProfileSettings>>,
}

impl SettingsManager {
    /// Create a new SettingsManager with the given app data directory
    pub fn new(app_data_dir: PathBuf) -> Self {
        let settings_path = app_data_dir.join("settings.json");
        Self {
            settings_path,
            app_data_dir,
            cache: RwLock::new(None),
            legacy_profile_settings: RwLock::new(None),
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

            // Decrypt sensitive fields first so legacy migration has access to plaintext
            self.decrypt_sensitive_fields(&mut user_value);

            // Extract legacy per-profile settings and remove legacy keys from settings.json
            let (legacy_profile_settings, legacy_removed) = extract_legacy_profile_settings(&mut user_value);
            if let Some(legacy) = legacy_profile_settings {
                if let Ok(mut guard) = self.legacy_profile_settings.write() {
                    if guard.is_none() {
                        *guard = Some(legacy);
                    }
                }
            }

            let defaults_value = serde_json::to_value(Settings::default())
                .map_err(|e| format!("Failed to build default settings: {e}"))?;

            let mut changed = merge_missing_settings(&mut user_value, &defaults_value);
            if legacy_removed {
                changed = true;
            }

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

    /// Take legacy per-profile settings extracted during settings load (if any).
    /// Consumes the cached legacy settings so they are only migrated once.
    pub fn take_legacy_profile_settings(&self) -> Option<ProfileSettings> {
        if let Ok(mut guard) = self.legacy_profile_settings.write() {
            guard.take()
        } else {
            None
        }
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
        if let Ok(mut legacy) = self.legacy_profile_settings.write() {
            *legacy = None;
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

fn extract_legacy_profile_settings(value: &mut Value) -> (Option<ProfileSettings>, bool) {
    let map = match value.as_object_mut() {
        Some(map) => map,
        None => return (None, false),
    };

    let mut legacy = ProfileSettings::default();
    let mut found = false;
    let mut removed = false;

    fn mark(found: &mut bool, removed: &mut bool) {
        *found = true;
        *removed = true;
    }

    fn take_string(
        map: &mut serde_json::Map<String, Value>,
        key: &str,
        found: &mut bool,
        removed: &mut bool,
    ) -> Option<String> {
        map.remove(key).and_then(|val| {
            mark(found, removed);
            match val {
                Value::String(s) => Some(s),
                Value::Number(n) => Some(n.to_string()),
                Value::Bool(b) => Some(b.to_string()),
                _ => None,
            }
        })
    }

    fn take_bool(
        map: &mut serde_json::Map<String, Value>,
        key: &str,
        found: &mut bool,
        removed: &mut bool,
    ) -> Option<bool> {
        map.remove(key).and_then(|val| {
            mark(found, removed);
            match val {
                Value::Bool(b) => Some(b),
                Value::String(s) => s.parse::<bool>().ok(),
                _ => None,
            }
        })
    }

    fn take_u16(
        map: &mut serde_json::Map<String, Value>,
        key: &str,
        found: &mut bool,
        removed: &mut bool,
    ) -> Option<u16> {
        map.remove(key).and_then(|val| {
            mark(found, removed);
            match val {
                Value::Number(n) => n.as_u64().and_then(|v| u16::try_from(v).ok()),
                Value::String(s) => s.parse::<u16>().ok(),
                _ => None,
            }
        })
    }

    fn take_u32(
        map: &mut serde_json::Map<String, Value>,
        key: &str,
        found: &mut bool,
        removed: &mut bool,
    ) -> Option<u32> {
        map.remove(key).and_then(|val| {
            mark(found, removed);
            match val {
                Value::Number(n) => n.as_u64().and_then(|v| u32::try_from(v).ok()),
                Value::String(s) => s.parse::<u32>().ok(),
                _ => None,
            }
        })
    }

    fn take_i64(
        map: &mut serde_json::Map<String, Value>,
        key: &str,
        found: &mut bool,
        removed: &mut bool,
    ) -> Option<i64> {
        map.remove(key).and_then(|val| {
            mark(found, removed);
            match val {
                Value::Number(n) => n.as_i64(),
                Value::String(s) => s.parse::<i64>().ok(),
                _ => None,
            }
        })
    }

    fn take_obs_direction(
        map: &mut serde_json::Map<String, Value>,
        key: &str,
        found: &mut bool,
        removed: &mut bool,
    ) -> Option<ObsIntegrationDirection> {
        map.remove(key).and_then(|val| {
            mark(found, removed);
            serde_json::from_value::<ObsIntegrationDirection>(val).ok()
        })
    }

    if let Some(value) = take_string(map, "themeId", &mut found, &mut removed) {
        legacy.theme_id = value;
    }
    if let Some(value) = take_string(map, "language", &mut found, &mut removed) {
        legacy.language = value;
    }
    if let Some(value) = take_bool(map, "showNotifications", &mut found, &mut removed) {
        legacy.show_notifications = value;
    }
    if let Some(value) = take_bool(map, "encryptStreamKeys", &mut found, &mut removed) {
        legacy.encrypt_stream_keys = value;
    }

    if let Some(value) = take_bool(map, "backendRemoteEnabled", &mut found, &mut removed) {
        legacy.backend.remote_enabled = value;
    }
    if let Some(value) = take_bool(map, "backendUiEnabled", &mut found, &mut removed) {
        legacy.backend.ui_enabled = value;
    }
    if let Some(value) = take_string(map, "backendHost", &mut found, &mut removed) {
        legacy.backend.host = value;
    }
    if let Some(value) = take_u16(map, "backendPort", &mut found, &mut removed) {
        legacy.backend.port = value;
    }
    if let Some(value) = take_string(map, "backendToken", &mut found, &mut removed) {
        legacy.backend.token = value;
    }

    if let Some(value) = take_string(map, "obsHost", &mut found, &mut removed) {
        legacy.obs.host = value;
    }
    if let Some(value) = take_u16(map, "obsPort", &mut found, &mut removed) {
        legacy.obs.port = value;
    }
    if let Some(value) = take_string(map, "obsPassword", &mut found, &mut removed) {
        legacy.obs.password = value;
    }
    if let Some(value) = take_bool(map, "obsUseAuth", &mut found, &mut removed) {
        legacy.obs.use_auth = value;
    }
    if let Some(value) = take_obs_direction(map, "obsDirection", &mut found, &mut removed) {
        legacy.obs.direction = value;
    }
    if let Some(value) = take_bool(map, "obsAutoConnect", &mut found, &mut removed) {
        legacy.obs.auto_connect = value;
    }

    if let Some(value) = take_bool(map, "discordWebhookEnabled", &mut found, &mut removed) {
        legacy.discord.webhook_enabled = value;
    }
    if let Some(value) = take_string(map, "discordWebhookUrl", &mut found, &mut removed) {
        legacy.discord.webhook_url = value;
    }
    if let Some(value) = take_string(map, "discordGoLiveMessage", &mut found, &mut removed) {
        legacy.discord.go_live_message = value;
    }
    if let Some(value) = take_bool(map, "discordCooldownEnabled", &mut found, &mut removed) {
        legacy.discord.cooldown_enabled = value;
    }
    if let Some(value) = take_u32(map, "discordCooldownSeconds", &mut found, &mut removed) {
        legacy.discord.cooldown_seconds = value;
    }
    if let Some(value) = take_string(map, "discordImagePath", &mut found, &mut removed) {
        legacy.discord.image_path = value;
    }

    if let Some(value) = take_string(map, "chatTwitchChannel", &mut found, &mut removed) {
        legacy.chat.twitch_channel = value;
    }
    if let Some(value) = take_string(map, "chatYoutubeChannelId", &mut found, &mut removed) {
        legacy.chat.youtube_channel_id = value;
    }
    if let Some(value) = take_string(map, "chatYoutubeApiKey", &mut found, &mut removed) {
        legacy.chat.youtube_api_key = value;
    }
    if let Some(value) = take_bool(map, "chatTwitchSendEnabled", &mut found, &mut removed) {
        legacy.chat.twitch_send_enabled = value;
    }
    if let Some(value) = take_bool(map, "chatYoutubeSendEnabled", &mut found, &mut removed) {
        legacy.chat.youtube_send_enabled = value;
    }
    if let Some(value) = take_bool(map, "chatSendAllEnabled", &mut found, &mut removed) {
        legacy.chat.send_all_enabled = value;
    }
    if let Some(value) = take_bool(map, "chatCrosspostEnabled", &mut found, &mut removed) {
        legacy.chat.crosspost_enabled = value;
    }
    if let Some(value) = take_bool(map, "youtubeUseApiKey", &mut found, &mut removed) {
        legacy.chat.youtube_use_api_key = value;
    }

    if let Some(value) = take_string(map, "twitchOauthAccessToken", &mut found, &mut removed) {
        legacy.oauth.twitch.access_token = value;
    }
    if let Some(value) = take_string(map, "twitchOauthRefreshToken", &mut found, &mut removed) {
        legacy.oauth.twitch.refresh_token = value;
    }
    if let Some(value) = take_i64(map, "twitchOauthExpiresAt", &mut found, &mut removed) {
        legacy.oauth.twitch.expires_at = value;
    }
    if let Some(value) = take_string(map, "twitchOauthUserId", &mut found, &mut removed) {
        legacy.oauth.twitch.user_id = value;
    }
    if let Some(value) = take_string(map, "twitchOauthUsername", &mut found, &mut removed) {
        legacy.oauth.twitch.username = value;
    }
    if let Some(value) = take_string(map, "twitchOauthDisplayName", &mut found, &mut removed) {
        legacy.oauth.twitch.display_name = value;
    }

    if let Some(value) = take_string(map, "youtubeOauthAccessToken", &mut found, &mut removed) {
        legacy.oauth.youtube.access_token = value;
    }
    if let Some(value) = take_string(map, "youtubeOauthRefreshToken", &mut found, &mut removed) {
        legacy.oauth.youtube.refresh_token = value;
    }
    if let Some(value) = take_i64(map, "youtubeOauthExpiresAt", &mut found, &mut removed) {
        legacy.oauth.youtube.expires_at = value;
    }
    if let Some(value) = take_string(map, "youtubeOauthChannelId", &mut found, &mut removed) {
        legacy.oauth.youtube.user_id = value;
        legacy.oauth.youtube.username = legacy.oauth.youtube.user_id.clone();
    }
    if let Some(value) = take_string(map, "youtubeOauthChannelName", &mut found, &mut removed) {
        legacy.oauth.youtube.display_name = value;
    }

    if found {
        (Some(legacy), removed)
    } else {
        (None, removed)
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
