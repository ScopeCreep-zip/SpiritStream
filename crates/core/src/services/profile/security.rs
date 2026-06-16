//! Encryption boundary for profile persistence.
//!
//! Owns the per-field encryption of stream keys, OBS password, Discord
//! webhook, backend token, YouTube API key, and OAuth tokens; plus the
//! whole-file Argon2id + AES-GCM-SIV envelope used when a password is
//! supplied. Save / load operations live here because their orchestration
//! and the encryption helpers are tightly coupled — splitting them would
//! risk an encryption-boundary mismatch.

use super::io::{ENCRYPTED_MAGIC_LEN, ENCRYPTED_MAGIC_V2};
use super::secret_fields::{visit_secret_fields, SecretFieldKind};
use super::validation::validate_profile_name;
use crate::errors::{CoreError, ValidationIssue};
use crate::models::Profile;
use crate::services::Encryption;

impl super::ProfileManager {
    /// Encrypt every machine-key-protected field of `profile` in place.
    ///
    /// The field inventory lives in [`visit_secret_fields`] — the same
    /// walker machine-key rotation uses, so save and rotation can never
    /// disagree about which fields are encrypted. Stream keys honor the
    /// per-profile `encrypt_stream_keys` flag; sensitive settings, OAuth
    /// tokens, and PII blocklist entries are always wrapped (the PII
    /// promise dates to O.11b: plaintext profiles must never store real
    /// names / deadnames / hometowns as raw JSON on disk). Idempotent —
    /// already-wrapped values pass through unchanged.
    pub(super) fn encrypt_secret_fields(
        &self,
        profile: &mut Profile,
        encrypt_stream_keys: bool,
    ) -> Result<(), CoreError> {
        visit_secret_fields(profile, |kind, field| {
            if kind == SecretFieldKind::StreamKey && !encrypt_stream_keys {
                return Ok(());
            }
            if !field.is_empty() && !Encryption::is_stream_key_encrypted(field) {
                *field = Encryption::encrypt_stream_key(field, &self.app_data_dir)?;
            }
            Ok(())
        })
    }

    /// Inverse of [`Self::encrypt_secret_fields`] across every secret
    /// kind. Plaintext values (legacy profiles) pass through unchanged
    /// so they get wrapped on the next save.
    pub(super) fn decrypt_secret_fields(&self, profile: &mut Profile) -> Result<(), CoreError> {
        visit_secret_fields(profile, |_, field| {
            if Encryption::is_stream_key_encrypted(field) {
                *field = Encryption::decrypt_stream_key(field, &self.app_data_dir)?;
            }
            Ok(())
        })
    }

    /// Unwrap only the PII blocklist entries. The inner `load()` path
    /// needs the blocklist in cleartext on every load (the PII filter
    /// compares against it) while leaving the other fields exactly as
    /// stored.
    pub(super) fn decrypt_pii_blocklist(&self, profile: &mut Profile) -> Result<(), CoreError> {
        visit_secret_fields(profile, |kind, field| {
            if kind == SecretFieldKind::PiiEntry && Encryption::is_stream_key_encrypted(field) {
                *field = Encryption::decrypt_stream_key(field, &self.app_data_dir)?;
            }
            Ok(())
        })
    }

    #[cfg(test)]
    pub(crate) fn test_encrypt_profile_settings(
        &self,
        profile: &mut Profile,
    ) -> Result<(), CoreError> {
        self.encrypt_secret_fields(profile, false)
    }

    #[cfg(test)]
    pub(crate) fn test_decrypt_profile_settings(
        &self,
        profile: &mut Profile,
    ) -> Result<(), CoreError> {
        self.decrypt_secret_fields(profile)
    }

    /// Save a profile with optional password-based encryption.
    ///
    /// One normalization happens on the way in:
    /// 1. `PlatformRegistry::normalize_url()` runs on every stream-target URL
    ///    so platform-specific path prefixes are consistent regardless of
    ///    what the user typed.
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

        // Server-authoritative incoming URL — whatever the client sent
        // for `input.url` is recomputed from the constituent fields.
        profile_to_save.input.refresh_url();

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

        // Canonicalise the blocklist (trim / drop empties / dedupe)
        // before wrapping. Lives here — not in any frontend — so every
        // writer produces the same shape. Already-encrypted entries
        // pass through untouched (base64 has no leading/trailing
        // whitespace to trim).
        super::validation::normalize_pii_blocklist(&mut profile_to_save.pii_blocklist);

        // One walker-driven sweep covers stream keys (gated on the
        // per-profile flag), sensitive settings, OAuth tokens, and the
        // PII blocklist. The blocklist is wrapped even for password-
        // encrypted profiles (idempotent), so a profile flipping between
        // password+no-password is always at-rest-protected on disk —
        // real names / deadnames / hometowns never hit disk as raw JSON.
        self.encrypt_secret_fields(&mut profile_to_save, encrypt_keys)?;

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
        if let Some(audit) = self.audit() {
            if let Err(e) = audit.record(crate::services::AuditAction::ProfileSaved {
                name: profile.name.clone(),
            }) {
                log::error!("profile_manager failed to append ProfileSaved audit entry: {e}");
            }
        }
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

        // Walker-driven sweep: stream keys, sensitive settings, OAuth
        // tokens, and the PII blocklist all unwrap here so callers see
        // plaintext. Legacy plaintext entries pass through unchanged and
        // get wrapped on the next save.
        self.decrypt_secret_fields(&mut profile)?;

        // Profiles saved before `input.url` existed deserialize with an
        // empty string — recompute so every consumer sees the
        // authoritative URL.
        profile.input.refresh_url();

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
mod tests {
    use super::*;
    use crate::models::ProfileSettings;
    use crate::services::Encryption;
    use crate::services::ProfileManager;
    use tempfile::TempDir;

    fn profile_with_oauth_tokens() -> Profile {
        let mut settings = ProfileSettings::default();
        settings.oauth.twitch.access_token = "tw-access".into();
        settings.oauth.twitch.refresh_token = "tw-refresh".into();
        settings.oauth.youtube.access_token = "yt-access".into();
        settings.oauth.youtube.refresh_token = "yt-refresh".into();
        settings.oauth.kick.access_token = "kk-access".into();
        settings.oauth.kick.refresh_token = "kk-refresh".into();
        settings.oauth.facebook.access_token = "fb-access".into();
        settings.oauth.facebook.refresh_token = "fb-refresh".into();
        Profile {
            id: "g1-test".into(),
            name: "g1-test".into(),
            encrypted: false,
            input: crate::models::RtmpInput::default(),
            output_groups: Vec::new(),
            settings,
            pii_blocklist: Vec::new(),
            pii_fuzzy: false,
            anonymous_logging: true,
            anonymous_salt: String::new(),
        }
    }

    /// G1 regression: pre-fix, only twitch + youtube OAuth tokens were
    /// encrypted. Kick + Facebook ended up on disk in plaintext even
    /// when `encrypt_stream_keys` was on. This test pins that all four
    /// providers round-trip through the encrypt path.
    #[tokio::test]
    async fn encrypts_oauth_tokens_for_all_four_providers() {
        let dir = TempDir::new().unwrap();
        let mgr = ProfileManager::new(dir.path().to_path_buf());

        let mut p = profile_with_oauth_tokens();
        mgr.test_encrypt_profile_settings(&mut p).unwrap();

        for (provider, account) in [
            ("twitch", &p.settings.oauth.twitch),
            ("youtube", &p.settings.oauth.youtube),
            ("kick", &p.settings.oauth.kick),
            ("facebook", &p.settings.oauth.facebook),
        ] {
            assert!(
                Encryption::is_stream_key_encrypted(&account.access_token),
                "{provider} access_token not encrypted: {}",
                account.access_token,
            );
            assert!(
                Encryption::is_stream_key_encrypted(&account.refresh_token),
                "{provider} refresh_token not encrypted: {}",
                account.refresh_token,
            );
        }

        // Round trip back to plaintext.
        mgr.test_decrypt_profile_settings(&mut p).unwrap();
        assert_eq!(p.settings.oauth.kick.access_token, "kk-access");
        assert_eq!(p.settings.oauth.facebook.refresh_token, "fb-refresh");
    }

    /// O.11b regression: a plaintext profile's `pii_blocklist` used to
    /// hit disk as raw JSON strings — the threat model's worst case
    /// (real name / deadname / hometown plain-text on disk). Wrap each
    /// entry with the `ENC2::` envelope so an attacker who reads the
    /// `.json` file sees ciphertext, not phrases.
    #[tokio::test]
    async fn pii_blocklist_is_wrapped_on_save_for_plaintext_profile() {
        let dir = TempDir::new().unwrap();
        let mgr = ProfileManager::new(dir.path().to_path_buf());

        let mut p = profile_with_oauth_tokens();
        p.name = "pii-test".into();
        p.id = "pii-test".into();
        p.pii_blocklist = vec![
            "Alice Smith".into(),
            "alice@example.com".into(),
            "123 Main St".into(),
        ];

        // Save plaintext (no password) — the on-disk JSON must not
        // contain any of the raw phrases.
        mgr.save_with_key_encryption(&p, None).await.unwrap();
        let on_disk =
            std::fs::read_to_string(dir.path().join("profiles").join("pii-test.json")).unwrap();
        for phrase in ["Alice Smith", "alice@example.com", "123 Main St"] {
            assert!(
                !on_disk.contains(phrase),
                "PII phrase {phrase:?} leaked into plaintext profile JSON",
            );
        }
        assert!(
            on_disk.contains("ENC2::"),
            "Expected ENC2:: wrapping on PII entries in plaintext profile",
        );

        // Load roundtrip: callers must see the original plaintext phrases.
        let loaded = mgr
            .load_with_key_decryption("pii-test", None)
            .await
            .unwrap();
        assert_eq!(
            loaded.pii_blocklist,
            vec![
                "Alice Smith".to_string(),
                "alice@example.com".to_string(),
                "123 Main St".to_string(),
            ],
        );
    }

    /// G2 regression: `ProfileSaved` was a defined-not-emitted variant.
    /// Wiring `set_audit_log` + emission inside `save_with_key_encryption`
    /// makes every save observable in the HMAC chain.
    #[tokio::test]
    async fn save_emits_profile_saved_audit_entry() {
        use crate::services::{AuditAction, AuditLogService};
        use std::sync::Arc;
        let dir = TempDir::new().unwrap();
        let mgr = ProfileManager::new(dir.path().to_path_buf());
        let audit = Arc::new(AuditLogService::new_for_tests(dir.path().to_path_buf()).unwrap());
        mgr.set_audit_log(audit.clone());

        let mut p = profile_with_oauth_tokens();
        p.input.port = 19350; // avoid collision with any concurrent test
        mgr.save_with_key_encryption(&p, None).await.unwrap();

        let entries = audit.entries().unwrap();
        let saved = entries
            .iter()
            .filter(|e| matches!(e.action, AuditAction::ProfileSaved { .. }))
            .count();
        assert!(saved >= 1, "expected at least one ProfileSaved entry");
    }

    /// G2 regression: `ProfileDeleted` was also defined-not-emitted.
    #[tokio::test]
    async fn delete_emits_profile_deleted_audit_entry() {
        use crate::services::{AuditAction, AuditLogService};
        use std::sync::Arc;
        let dir = TempDir::new().unwrap();
        let mgr = ProfileManager::new(dir.path().to_path_buf());
        let audit = Arc::new(AuditLogService::new_for_tests(dir.path().to_path_buf()).unwrap());
        mgr.set_audit_log(audit.clone());

        let mut p = profile_with_oauth_tokens();
        p.input.port = 19351;
        mgr.save_with_key_encryption(&p, None).await.unwrap();
        mgr.delete("g1-test").await.unwrap();

        let entries = audit.entries().unwrap();
        let deleted = entries
            .iter()
            .filter(|e| matches!(e.action, AuditAction::ProfileDeleted { .. }))
            .count();
        assert_eq!(deleted, 1, "expected exactly one ProfileDeleted entry");
    }
}
