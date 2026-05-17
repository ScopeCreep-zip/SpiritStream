// ProfileManager Service
// Handles profile persistence and encryption
use crate::errors::{CoreError, ValidationIssue};
use crate::models::{Profile, ProfileSummary, RtmpInput};
use crate::services::Encryption;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::PathBuf;

/// Plan-driven `ProfileActivated` event payload — the consolidated state the
/// transport emits whenever a profile is activated. Replaces the frontend
/// `applyProfileSettings` cascade that used to assemble this from individual
/// `Profile.settings` fields. (every UI store listens for one event
/// instead of being driven by a synchronous procedural cascade.)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct ProfileActivatedEvent {
    pub name: String,
    pub theme_id: String,
    pub language: String,
    pub show_notifications: bool,
    pub encrypt_stream_keys: bool,
    pub obs: ActivatedObs,
    pub chat: crate::models::ChatSettings,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct ActivatedObs {
    pub host: String,
    pub port: u16,
    pub use_auth: bool,
    pub direction: crate::models::ObsIntegrationDirection,
    pub auto_connect: bool,
}

impl ProfileActivatedEvent {
    /// Public helper for transports that need to construct the event shape
    /// without re-running the full `activate()` orchestration — e.g. when
    /// re-emitting after a save to an already-active profile.
    pub fn from_profile_public(profile: &Profile) -> Self {
        Self::from_profile(profile)
    }

    fn from_profile(profile: &Profile) -> Self {
        let s = &profile.settings;
        Self {
            name: profile.name.clone(),
            theme_id: s.theme_id.clone(),
            language: s.language.clone(),
            show_notifications: s.show_notifications,
            encrypt_stream_keys: s.encrypt_stream_keys,
            obs: ActivatedObs {
                host: s.obs.host.clone(),
                port: s.obs.port,
                use_auth: s.obs.use_auth,
                direction: s.obs.direction,
                auto_connect: s.obs.auto_connect,
            },
            chat: s.chat.clone(),
        }
    }
}

// Magic bytes that identify encrypted profile files. Legacy installs
// produced `MGLA` blobs (AES-256-GCM body); current writers produce `MGL2`
// blobs (AES-256-GCM-SIV body). `load()` reads either; `save()` writes V2.
pub(crate) const ENCRYPTED_MAGIC_V1: &[u8] = b"MGLA";
pub(crate) const ENCRYPTED_MAGIC_V2: &[u8] = b"MGL2";
pub(crate) const ENCRYPTED_MAGIC_LEN: usize = 4;

/// Validate profile name to prevent path traversal attacks.
///
/// Returns `CoreError::ValidationFailed` with stable issue codes so callers
/// (CLI, frontend) can localize the message or branch programmatically.
fn validate_profile_name(name: &str) -> Result<(), CoreError> {
    let mut reasons: Vec<ValidationIssue> = Vec::new();
    if name.is_empty() {
        reasons.push(ValidationIssue {
            code: "profile_name_empty".into(),
            message: "Profile name cannot be empty.".into(),
            path: Some("/name".into()),
        });
    }
    if name.contains('/') || name.contains('\\') {
        reasons.push(ValidationIssue {
            code: "profile_name_path_separator".into(),
            message: "Profile name cannot contain path separators.".into(),
            path: Some("/name".into()),
        });
    }
    if name.contains("..") {
        reasons.push(ValidationIssue {
            code: "profile_name_path_traversal".into(),
            message: "Profile name cannot contain '..'.".into(),
            path: Some("/name".into()),
        });
    }
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == ' ')
    {
        reasons.push(ValidationIssue {
            code: "profile_name_charset".into(),
            message:
                "Profile name can only contain letters, numbers, spaces, underscores, and hyphens."
                    .into(),
            path: Some("/name".into()),
        });
    }
    if name.len() > 100 {
        reasons.push(ValidationIssue {
            code: "profile_name_too_long".into(),
            message: "Profile name is too long (max 100 characters).".into(),
            path: Some("/name".into()),
        });
    }
    if reasons.is_empty() {
        Ok(())
    } else {
        Err(CoreError::ValidationFailed { reasons })
    }
}

/// Manages profile storage and retrieval
pub struct ProfileManager {
    profiles_dir: PathBuf,
    app_data_dir: PathBuf,
    order_index_dir: PathBuf,
}

/// Discord webhook cooldown upper bound, in seconds. Zero is allowed
/// ("no rate limiting between notifications"); beyond 24 hours the
/// user has effectively disabled the notification and a typed setting
/// would be misleading.
pub const DISCORD_COOLDOWN_SECONDS_MAX: u32 = 86_400;

/// Backend bind port lower bound. Port 0 is reserved and never valid
/// for binding.
pub const BACKEND_PORT_MIN: u16 = 1;

impl ProfileManager {
    /// Enforce bounds on profile-scoped settings that now live in
    /// `ProfileSettings`.
    /// Out-of-range values produce a single `CoreError::ValidationFailed`
    /// carrying every offending field — callers get a complete list.
    fn validate_profile_settings_bounds(
        settings: &crate::models::ProfileSettings,
    ) -> Result<(), CoreError> {
        let mut issues = Vec::new();
        if settings.backend.port < BACKEND_PORT_MIN {
            issues.push(ValidationIssue {
                code: "backend_port_out_of_range".into(),
                message: format!(
                    "backend.port must be in [{BACKEND_PORT_MIN}, 65535], got {}",
                    settings.backend.port
                ),
                path: Some("/settings/backend/port".into()),
            });
        }
        if settings.discord.cooldown_seconds > DISCORD_COOLDOWN_SECONDS_MAX {
            issues.push(ValidationIssue {
                code: "discord_cooldown_seconds_out_of_range".into(),
                message: format!(
                    "discord.cooldown_seconds must be in [0, {DISCORD_COOLDOWN_SECONDS_MAX}], got {}",
                    settings.discord.cooldown_seconds
                ),
                path: Some("/settings/discord/cooldownSeconds".into()),
            });
        }
        if issues.is_empty() {
            Ok(())
        } else {
            Err(CoreError::ValidationFailed { reasons: issues })
        }
    }

    /// Create a new ProfileManager with the given app data directory
    pub fn new(app_data_dir: PathBuf) -> Self {
        let profiles_dir = app_data_dir.join("profiles");
        let order_index_dir = app_data_dir.join("indexes");
        std::fs::create_dir_all(&profiles_dir).ok();
        std::fs::create_dir_all(&order_index_dir).ok();
        Self {
            profiles_dir,
            app_data_dir,
            order_index_dir,
        }
    }

    // To read order indexes for drag and drop on profiles
    pub fn read_order_index_map(&self) -> Result<HashMap<String, i32>, CoreError> {
        let indexes_path = self.order_index_dir.join("order_indexes.json");

        if !indexes_path.exists() {
            let empty: HashMap<String, i32> = HashMap::new();
            let content = serde_json::to_string_pretty(&empty)?;
            std::fs::write(&indexes_path, content)?;
            return Ok(empty);
        }

        let content = std::fs::read_to_string(&indexes_path)?;
        let map: HashMap<String, i32> = serde_json::from_str(&content)?;
        Ok(map)
    }

    pub fn write_order_index_map(&self, map: &HashMap<String, i32>) -> Result<(), CoreError> {
        let path = self.order_index_dir.join("order_indexes.json");
        let tmp = self.order_index_dir.join("order_indexes.json.tmp");

        let content = serde_json::to_string_pretty(map)?;
        std::fs::write(&tmp, content)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// This method can eventually be removed, it's purpose is to add order_index
    /// to profiles that were created before order_index was introduced
    pub async fn ensure_order_indexes(&self) -> Result<HashMap<String, i32>, CoreError> {
        let names = self.get_all_names().await?;
        let mut map = self.read_order_index_map()?;

        let mut max = map.values().copied().max().unwrap_or(0);
        max = ((max + 9) / 10) * 10;

        let mut changed = false;

        for name in names.clone() {
            match map.entry(name.clone()) {
                Entry::Vacant(e) => {
                    max += 10;
                    e.insert(max);
                    changed = true;
                }
                Entry::Occupied(_) => {}
            }
        }
        map.retain(|k, _| names.contains(k));
        if changed {
            self.write_order_index_map(&map)?;
        }

        Ok(map)
    }

    /// Get all profile names from the profiles directory
    pub async fn get_all_names(&self) -> Result<Vec<String>, CoreError> {
        let mut names = Vec::new();

        let entries = std::fs::read_dir(&self.profiles_dir)?;

        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_stem() {
                // Accept both .json (unencrypted) and .mgs (encrypted) files
                let ext = path.extension().and_then(|e| e.to_str());
                if ext == Some("json") || ext == Some("mgs") {
                    names.push(name.to_string_lossy().to_string());
                }
            }
        }

        Ok(names)
    }

    /// Get summaries of all profiles (for list display)
    /// Includes services list for each profile (Story 1.1, 4.1, 4.2)
    pub async fn get_all_summaries(&self) -> Result<Vec<ProfileSummary>, CoreError> {
        let names = self.get_all_names().await?;
        let order_map = self.ensure_order_indexes().await?; //<- Can eventually be replaced

        // ensure_order_indexes can be replaced with this below eventually
        // the purpose for ensure_order_indexes is to keep things from breaking
        // if the user had profiles saved before merging this code,
        // this will ensure the index is added to previously saved
        // profiles
        // eventual replacement -> let order_map = self.read_order_index_map()?;

        let mut summaries = Vec::new();

        for name in names {
            let is_encrypted = self.is_encrypted(&name);

            // Try to load the profile (unencrypted profiles only)
            // Encrypted profiles show minimal info since we can't read them without password
            if let Ok(profile) = self.load(&name, None).await {
                summaries.push(profile.to_summary(is_encrypted));
            } else if is_encrypted {
                // For encrypted profiles we can't read, show minimal summary
                summaries.push(ProfileSummary {
                    id: String::new(),
                    name: name.clone(),
                    resolution: "?".to_string(),
                    bitrate: 0,
                    target_count: 0,
                    services: Vec::new(),
                    is_encrypted: true,
                });
            }
        }
        summaries.sort_by_key(|s| order_map.get(&s.name).copied().unwrap_or(i32::MAX));
        Ok(summaries)
    }

    /// Load a profile by name.
    /// If password is provided, will attempt to decrypt.
    /// If no password, will try unencrypted first, and return
    /// `CoreError::PasswordRequired` if the profile is encrypted.
    pub async fn load(&self, name: &str, password: Option<&str>) -> Result<Profile, CoreError> {
        validate_profile_name(name)?;

        let encrypted_path = self.profiles_dir.join(format!("{name}.mgs"));
        let json_path = self.profiles_dir.join(format!("{name}.json"));

        if encrypted_path.exists() {
            let data = std::fs::read(&encrypted_path)?;

            if data.len() < ENCRYPTED_MAGIC_LEN {
                return Err(CoreError::Internal {
                    context: format!("invalid encrypted profile format: {name}"),
                });
            }
            let magic = &data[..ENCRYPTED_MAGIC_LEN];
            let body = &data[ENCRYPTED_MAGIC_LEN..];

            // Treat empty-string password the same as no password — clients
            // sending `{"password": ""}` for an encrypted profile should get
            // `PasswordRequired` (so the UI prompts), not `PasswordIncorrect`
            // (which suggests they typed the wrong key).
            let password = match password {
                Some(p) if !p.is_empty() => p,
                _ => {
                    return Err(CoreError::PasswordRequired {
                        name: name.to_string(),
                    })
                }
            };

            let decrypted = if magic == ENCRYPTED_MAGIC_V2 {
                Encryption::decrypt_v2(body, password)?
            } else if magic == ENCRYPTED_MAGIC_V1 {
                Encryption::decrypt_v1(body, password)?
            } else {
                return Err(CoreError::Internal {
                    context: format!("invalid encrypted profile format: {name}"),
                });
            };

            let content = String::from_utf8(decrypted).map_err(|e| CoreError::Internal {
                context: format!("invalid utf-8 in decrypted profile: {e}"),
            })?;

            // file extension is the authoritative encryption flag
            // — overwrite whatever was stored inside the blob so the loaded
            // Profile can't lie about its own encryption state.
            let mut profile: Profile = serde_json::from_str(&content)?;
            profile.encrypted = true;
            return Ok(profile);
        }

        if json_path.exists() {
            let content = std::fs::read_to_string(&json_path)?;
            let mut profile: Profile = serde_json::from_str(&content)?;
            profile.encrypted = false;
            return Ok(profile);
        }

        Err(CoreError::ProfileNotFound {
            name: name.to_string(),
        })
    }

    /// Delete a profile by name (both encrypted and unencrypted versions).
    pub async fn delete(&self, name: &str) -> Result<(), CoreError> {
        log::info!("Deleting profile: {name}");

        validate_profile_name(name)?;

        let json_path = self.profiles_dir.join(format!("{name}.json"));
        let mgs_path = self.profiles_dir.join(format!("{name}.mgs"));

        let mut deleted = false;

        if json_path.exists() {
            std::fs::remove_file(&json_path)?;
            deleted = true;
        }

        if mgs_path.exists() {
            std::fs::remove_file(&mgs_path)?;
            deleted = true;
        }

        if deleted {
            log::info!("Profile deleted successfully: {name}");
            Ok(())
        } else {
            log::warn!("Profile not found for deletion: {name}");
            Err(CoreError::ProfileNotFound {
                name: name.to_string(),
            })
        }
    }

    /// Check if a profile is encrypted
    /// Returns false for invalid profile names (fails safely)
    pub fn is_encrypted(&self, name: &str) -> bool {
        // Validate profile name to prevent path traversal attacks
        // For this method, we return false for invalid names (fail safely)
        if validate_profile_name(name).is_err() {
            return false;
        }

        let mgs_path = self.profiles_dir.join(format!("{name}.mgs"));
        mgs_path.exists()
    }

    /// Validate that no *other* profile claims the same RTMP `(bindAddress, port)`
    /// pair. Returns `CoreError::PortConflict` when another profile owns the
    /// same input. The current profile (matched by `profile_id`) is excluded
    /// from the scan so saving the same profile with unchanged input is a
    /// no-op.
    ///
    /// Only unencrypted profiles can be scanned without a password; encrypted
    /// profiles are skipped (we cannot determine their input config without
    /// the user's password, and the rest of the rewrite assumes the user
    /// remembers their own conflicts).
    pub async fn validate_input_conflict(
        &self,
        profile_id: &str,
        input: &RtmpInput,
    ) -> Result<(), CoreError> {
        let names = self.get_all_names().await?;
        for name in names {
            if self.is_encrypted(&name) {
                continue;
            }
            let other = match self.load(&name, None).await {
                Ok(p) => p,
                Err(_) => continue,
            };
            if other.id == profile_id {
                continue;
            }
            if other.input.port == input.port && other.input.bind_address == input.bind_address {
                return Err(CoreError::PortConflict {
                    port: input.port,
                    owner: other.name,
                });
            }
        }
        Ok(())
    }

    /// Encrypt all stream keys in a profile
    fn encrypt_stream_keys(&self, profile: &mut Profile) -> Result<(), CoreError> {
        for group in &mut profile.output_groups {
            for target in &mut group.stream_targets {
                // Skip if already encrypted or empty
                if !target.stream_key.is_empty()
                    && !Encryption::is_stream_key_encrypted(&target.stream_key)
                {
                    target.stream_key =
                        Encryption::encrypt_stream_key(&target.stream_key, &self.app_data_dir)?;
                }
            }
        }
        Ok(())
    }

    /// Decrypt all stream keys in a profile
    fn decrypt_stream_keys(&self, profile: &mut Profile) -> Result<(), CoreError> {
        for group in &mut profile.output_groups {
            for target in &mut group.stream_targets {
                // Only decrypt if encrypted
                if Encryption::is_stream_key_encrypted(&target.stream_key) {
                    target.stream_key =
                        Encryption::decrypt_stream_key(&target.stream_key, &self.app_data_dir)?;
                }
            }
        }
        Ok(())
    }

    /// Encrypt sensitive fields in profile settings (OBS password, Discord webhook, backend token, YouTube API key, OAuth tokens)
    fn encrypt_profile_settings(&self, profile: &mut Profile) -> Result<(), CoreError> {
        // Encrypt OBS password
        if !profile.settings.obs.password.is_empty()
            && !Encryption::is_stream_key_encrypted(&profile.settings.obs.password)
        {
            profile.settings.obs.password =
                Encryption::encrypt_stream_key(&profile.settings.obs.password, &self.app_data_dir)?;
        }

        // Encrypt Discord webhook URL
        if !profile.settings.discord.webhook_url.is_empty()
            && !Encryption::is_stream_key_encrypted(&profile.settings.discord.webhook_url)
        {
            profile.settings.discord.webhook_url = Encryption::encrypt_stream_key(
                &profile.settings.discord.webhook_url,
                &self.app_data_dir,
            )?;
        }

        // Encrypt backend token
        if !profile.settings.backend.token.is_empty()
            && !Encryption::is_stream_key_encrypted(&profile.settings.backend.token)
        {
            profile.settings.backend.token = Encryption::encrypt_stream_key(
                &profile.settings.backend.token,
                &self.app_data_dir,
            )?;
        }

        // Encrypt YouTube API key (chat settings)
        if !profile.settings.chat.youtube_api_key.is_empty()
            && !Encryption::is_stream_key_encrypted(&profile.settings.chat.youtube_api_key)
        {
            profile.settings.chat.youtube_api_key = Encryption::encrypt_stream_key(
                &profile.settings.chat.youtube_api_key,
                &self.app_data_dir,
            )?;
        }

        // Encrypt OAuth tokens (per profile)
        if !profile.settings.oauth.twitch.access_token.is_empty()
            && !Encryption::is_stream_key_encrypted(&profile.settings.oauth.twitch.access_token)
        {
            profile.settings.oauth.twitch.access_token = Encryption::encrypt_stream_key(
                &profile.settings.oauth.twitch.access_token,
                &self.app_data_dir,
            )?;
        }
        if !profile.settings.oauth.twitch.refresh_token.is_empty()
            && !Encryption::is_stream_key_encrypted(&profile.settings.oauth.twitch.refresh_token)
        {
            profile.settings.oauth.twitch.refresh_token = Encryption::encrypt_stream_key(
                &profile.settings.oauth.twitch.refresh_token,
                &self.app_data_dir,
            )?;
        }
        if !profile.settings.oauth.youtube.access_token.is_empty()
            && !Encryption::is_stream_key_encrypted(&profile.settings.oauth.youtube.access_token)
        {
            profile.settings.oauth.youtube.access_token = Encryption::encrypt_stream_key(
                &profile.settings.oauth.youtube.access_token,
                &self.app_data_dir,
            )?;
        }
        if !profile.settings.oauth.youtube.refresh_token.is_empty()
            && !Encryption::is_stream_key_encrypted(&profile.settings.oauth.youtube.refresh_token)
        {
            profile.settings.oauth.youtube.refresh_token = Encryption::encrypt_stream_key(
                &profile.settings.oauth.youtube.refresh_token,
                &self.app_data_dir,
            )?;
        }

        Ok(())
    }

    /// Decrypt sensitive fields in profile settings (OBS password, Discord webhook, backend token, YouTube API key, OAuth tokens)
    fn decrypt_profile_settings(&self, profile: &mut Profile) -> Result<(), CoreError> {
        // Decrypt OBS password
        if Encryption::is_stream_key_encrypted(&profile.settings.obs.password) {
            profile.settings.obs.password =
                Encryption::decrypt_stream_key(&profile.settings.obs.password, &self.app_data_dir)?;
        }

        // Decrypt Discord webhook URL
        if Encryption::is_stream_key_encrypted(&profile.settings.discord.webhook_url) {
            profile.settings.discord.webhook_url = Encryption::decrypt_stream_key(
                &profile.settings.discord.webhook_url,
                &self.app_data_dir,
            )?;
        }

        // Decrypt backend token
        if Encryption::is_stream_key_encrypted(&profile.settings.backend.token) {
            profile.settings.backend.token = Encryption::decrypt_stream_key(
                &profile.settings.backend.token,
                &self.app_data_dir,
            )?;
        }

        // Decrypt YouTube API key (chat settings)
        if Encryption::is_stream_key_encrypted(&profile.settings.chat.youtube_api_key) {
            profile.settings.chat.youtube_api_key = Encryption::decrypt_stream_key(
                &profile.settings.chat.youtube_api_key,
                &self.app_data_dir,
            )?;
        }

        // Decrypt OAuth tokens (per profile)
        if Encryption::is_stream_key_encrypted(&profile.settings.oauth.twitch.access_token) {
            profile.settings.oauth.twitch.access_token = Encryption::decrypt_stream_key(
                &profile.settings.oauth.twitch.access_token,
                &self.app_data_dir,
            )?;
        }
        if Encryption::is_stream_key_encrypted(&profile.settings.oauth.twitch.refresh_token) {
            profile.settings.oauth.twitch.refresh_token = Encryption::decrypt_stream_key(
                &profile.settings.oauth.twitch.refresh_token,
                &self.app_data_dir,
            )?;
        }
        if Encryption::is_stream_key_encrypted(&profile.settings.oauth.youtube.access_token) {
            profile.settings.oauth.youtube.access_token = Encryption::decrypt_stream_key(
                &profile.settings.oauth.youtube.access_token,
                &self.app_data_dir,
            )?;
        }
        if Encryption::is_stream_key_encrypted(&profile.settings.oauth.youtube.refresh_token) {
            profile.settings.oauth.youtube.refresh_token = Encryption::decrypt_stream_key(
                &profile.settings.oauth.youtube.refresh_token,
                &self.app_data_dir,
            )?;
        }
        Ok(())
    }

    /// Save a profile with optional password-based encryption.
    ///
    /// Two normalizations happen on the way in:
    /// 1. `PlatformRegistry::normalize_url()` runs on every stream-target URL
    ///    so platform-specific path prefixes are consistent regardless of
    ///    what the user typed.
    /// 2. `validate_input_conflict()` verifies no other profile claims the
    ///    same RTMP `(bindAddress, port)`.
    ///
    /// Encryption uses the profile's own `settings.encrypt_stream_keys`
    /// (stream keys + sensitive settings fields) and the optional `password`
    /// (whole-file Argon2id + AES-GCM-SIV envelope).
    pub async fn save_with_key_encryption(
        &self,
        profile: &Profile,
        password: Option<&str>,
    ) -> Result<(), CoreError> {
        let encrypt_keys = profile.settings.encrypt_stream_keys;

        log::info!(
            "Saving profile: {} (encrypted: {}, stream keys encrypted: {})",
            profile.name,
            password.is_some(),
            encrypt_keys
        );

        validate_profile_name(&profile.name)?;
        Self::validate_profile_settings_bounds(&profile.settings)?;
        if profile.id.trim().is_empty() {
            return Err(CoreError::ValidationFailed {
                reasons: vec![ValidationIssue {
                    code: "profile_id_empty".into(),
                    message: "Profile id cannot be empty.".into(),
                    path: Some("/id".into()),
                }],
            });
        }

        // High-value-secret length floor for profile encryption. See
        // `crate::services::PROFILE_PASSWORD_MIN_LENGTH` for
        // the threat-model rationale (NIST SP 800-63B-3 +
        // OWASP 2024 for memorized secrets protecting credentials,
        // PII blocklists, and anonymous-mode salts).
        if let Some(pw) = password {
            if pw.len() < crate::services::PROFILE_PASSWORD_MIN_LENGTH {
                return Err(CoreError::PasswordTooShort {
                    min_length: crate::services::PROFILE_PASSWORD_MIN_LENGTH as u32,
                });
            }
        }

        self.validate_input_conflict(&profile.id, &profile.input)
            .await?;

        // Clone so we can normalize + encrypt without mutating the caller's profile.
        let mut profile_to_save = profile.clone();

        // keep `Profile.encrypted` in sync with the actual file
        // shape we're about to write. Without this, a client sending
        // `encrypted: false` + a password would persist a misleading flag
        // inside the encrypted blob — `is-encrypted` would then lie on the
        // next load.
        profile_to_save.encrypted = password.is_some();

        // Generate the per-profile pseudonymizer salt on first
        // save if the caller didn't supply one. Existing profiles loaded
        // from disk before this field existed also get a salt the next
        // time they're saved.
        profile_to_save.ensure_anonymous_salt();

        // PlatformRegistry::normalize_url() exists in core but was
        // never called on save. Pull it in. Server is now the authoritative
        // place URL normalization happens.
        let registry = crate::services::PlatformRegistry::new();
        for group in &mut profile_to_save.output_groups {
            for target in &mut group.stream_targets {
                let normalized = registry.normalize_url(&target.service, &target.url);
                target.url = normalized;
            }
        }

        if encrypt_keys {
            self.encrypt_stream_keys(&mut profile_to_save)?;
        }
        self.encrypt_profile_settings(&mut profile_to_save)?;

        let content = serde_json::to_string_pretty(&profile_to_save)?;

        if let Some(pwd) = password {
            let encrypted = Encryption::encrypt(content.as_bytes(), pwd)?;

            let mut data = Vec::with_capacity(ENCRYPTED_MAGIC_LEN + encrypted.len());
            data.extend_from_slice(ENCRYPTED_MAGIC_V2);
            data.extend_from_slice(&encrypted);

            // Profile files always written owner-only.
            let path = self.profiles_dir.join(format!("{}.mgs", profile.name));
            crate::services::write_owner_only_atomic(&path, &data)?;

            let json_path = self.profiles_dir.join(format!("{}.json", profile.name));
            if json_path.exists() {
                std::fs::remove_file(&json_path).ok();
            }
        } else {
            let path = self.profiles_dir.join(format!("{}.json", profile.name));
            crate::services::write_owner_only_atomic(&path, content.as_bytes())?;

            let mgs_path = self.profiles_dir.join(format!("{}.mgs", profile.name));
            if mgs_path.exists() {
                std::fs::remove_file(&mgs_path).ok();
            }
        }

        log::info!("Profile saved successfully: {}", profile.name);
        Ok(())
    }

    /// Load a profile and always decrypt stream keys and sensitive settings.
    pub async fn load_with_key_decryption(
        &self,
        name: &str,
        password: Option<&str>,
    ) -> Result<Profile, CoreError> {
        log::info!("Loading profile: {name}");
        let mut profile = self.load(name, password).await?;

        self.decrypt_stream_keys(&mut profile)?;
        self.decrypt_profile_settings(&mut profile)?;

        log::info!(
            "Profile loaded successfully: {} ({} output groups, {} total targets)",
            name,
            profile.output_groups.len(),
            profile
                .output_groups
                .iter()
                .map(|g| g.stream_targets.len())
                .sum::<usize>()
        );
        Ok(profile)
    }
}

#[cfg(test)]
mod permission_tests {
    use super::*;
    use crate::models::{ProfileSettings, RtmpInput};

    fn fresh_dir() -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "spiritstream-profile-perm-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(p.join("profiles")).unwrap();
        p
    }

    /// Saved profile files (`.json` plaintext and `.mgs`
    /// encrypted) must land at mode 0600 on Unix. Profile files contain
    /// stream keys, OAuth tokens, OBS passwords, and webhook URLs — even
    /// the encrypted variant is sensitive metadata.
    #[cfg(unix)]
    #[tokio::test]
    async fn save_writes_profile_at_0600() {
        use std::os::unix::fs::PermissionsExt;
        let data_dir = fresh_dir();
        let mgr = ProfileManager::new(data_dir.clone());
        let profile = Profile {
            id: "phase69-test".into(),
            name: "phase69-test".into(),
            encrypted: false,
            input: RtmpInput {
                input_type: "rtmp".into(),
                bind_address: "127.0.0.1".into(),
                port: 1935,
                application: "live".into(),
            },
            output_groups: vec![],
            settings: ProfileSettings::default(),
            pii_blocklist: vec![],
            pii_fuzzy: false,
            anonymous_logging: true,
            anonymous_salt: String::new(),
        };
        mgr.save_with_key_encryption(&profile, None)
            .await
            .expect("save plaintext profile");
        let json_path = mgr.profiles_dir.join("phase69-test.json");
        let perms = std::fs::metadata(&json_path).unwrap().permissions();
        assert_eq!(
            perms.mode() & 0o777,
            0o600,
            "plaintext profile .json must be owner-only",
        );
        // Password must be ≥ PROFILE_PASSWORD_MIN_LENGTH (12 chars) — that's
        // the threat-model floor for the profile-encryption key.
        mgr.save_with_key_encryption(&profile, Some("test-pw-twelve"))
            .await
            .expect("save encrypted profile");
        let mgs_path = mgr.profiles_dir.join("phase69-test.mgs");
        let perms = std::fs::metadata(&mgs_path).unwrap().permissions();
        assert_eq!(
            perms.mode() & 0o777,
            0o600,
            "encrypted profile .mgs must be owner-only",
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
