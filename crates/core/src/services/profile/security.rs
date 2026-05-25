//! Encryption boundary for profile persistence.
//!
//! Owns the per-field encryption of stream keys, OBS password, Discord
//! webhook, backend token, YouTube API key, and OAuth tokens; plus the
//! whole-file Argon2id + AES-GCM-SIV envelope used when a password is
//! supplied. Save / load operations live here because their orchestration
//! and the encryption helpers are tightly coupled — splitting them would
//! risk an encryption-boundary mismatch.

use crate::errors::{CoreError, ValidationIssue};
use crate::models::Profile;
use crate::services::Encryption;
use super::io::{ENCRYPTED_MAGIC_LEN, ENCRYPTED_MAGIC_V2};
use super::validation::validate_profile_name;

impl super::ProfileManager {
    /// Encrypt all stream keys in a profile.
    pub(super) fn encrypt_stream_keys(&self, profile: &mut Profile) -> Result<(), CoreError> {
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

    /// Decrypt all stream keys in a profile.
    pub(super) fn decrypt_stream_keys(&self, profile: &mut Profile) -> Result<(), CoreError> {
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

    /// Encrypt sensitive fields in profile settings (OBS password, Discord
    /// webhook, backend token, YouTube API key, OAuth tokens).
    pub(super) fn encrypt_profile_settings(&self, profile: &mut Profile) -> Result<(), CoreError> {
        if !profile.settings.obs.password.is_empty()
            && !Encryption::is_stream_key_encrypted(&profile.settings.obs.password)
        {
            profile.settings.obs.password =
                Encryption::encrypt_stream_key(&profile.settings.obs.password, &self.app_data_dir)?;
        }

        if !profile.settings.discord.webhook_url.is_empty()
            && !Encryption::is_stream_key_encrypted(&profile.settings.discord.webhook_url)
        {
            profile.settings.discord.webhook_url = Encryption::encrypt_stream_key(
                &profile.settings.discord.webhook_url,
                &self.app_data_dir,
            )?;
        }

        if !profile.settings.backend.token.is_empty()
            && !Encryption::is_stream_key_encrypted(&profile.settings.backend.token)
        {
            profile.settings.backend.token = Encryption::encrypt_stream_key(
                &profile.settings.backend.token,
                &self.app_data_dir,
            )?;
        }

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

    /// Decrypt sensitive fields in profile settings (OBS password, Discord
    /// webhook, backend token, YouTube API key, OAuth tokens).
    pub(super) fn decrypt_profile_settings(&self, profile: &mut Profile) -> Result<(), CoreError> {
        if Encryption::is_stream_key_encrypted(&profile.settings.obs.password) {
            profile.settings.obs.password =
                Encryption::decrypt_stream_key(&profile.settings.obs.password, &self.app_data_dir)?;
        }

        if Encryption::is_stream_key_encrypted(&profile.settings.discord.webhook_url) {
            profile.settings.discord.webhook_url = Encryption::decrypt_stream_key(
                &profile.settings.discord.webhook_url,
                &self.app_data_dir,
            )?;
        }

        if Encryption::is_stream_key_encrypted(&profile.settings.backend.token) {
            profile.settings.backend.token = Encryption::decrypt_stream_key(
                &profile.settings.backend.token,
                &self.app_data_dir,
            )?;
        }

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
        // `crate::services::PROFILE_PASSWORD_MIN_LENGTH` for the threat-
        // model rationale (NIST SP 800-63B-3 + OWASP 2024 for memorized
        // secrets protecting credentials, PII blocklists, and anonymous-
        // mode salts).
        if let Some(pw) = password {
            if pw.len() < crate::services::PROFILE_PASSWORD_MIN_LENGTH {
                return Err(CoreError::PasswordTooShort {
                    min_length: crate::services::PROFILE_PASSWORD_MIN_LENGTH as u32,
                });
            }
        }

        self.validate_input_conflict(&profile.id, &profile.input)
            .await?;

        // Clone so we can normalize + encrypt without mutating the caller's
        // profile.
        let mut profile_to_save = profile.clone();

        // Keep `Profile.encrypted` in sync with the actual file shape we're
        // about to write. Without this, a client sending `encrypted: false`
        // + a password would persist a misleading flag inside the encrypted
        // blob — `is-encrypted` would then lie on the next load.
        profile_to_save.encrypted = password.is_some();

        // Generate the per-profile pseudonymizer salt on first save if the
        // caller didn't supply one. Existing profiles loaded from disk
        // before this field existed also get a salt the next time they're
        // saved.
        profile_to_save.ensure_anonymous_salt();

        // `PlatformRegistry::normalize_url()` exists in core but was never
        // called on save. Pull it in. Server is now the authoritative place
        // URL normalization happens.
        let registry = crate::services::PlatformRegistry::new()?;
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
