//! Profile file I/O: list, load (with encryption-aware decode), delete,
//! is-encrypted probe, RTMP input-conflict scan.

use super::validation::validate_profile_name;
use crate::errors::CoreError;
use crate::models::{Profile, ProfileSummary, RtmpInput};
use crate::services::Encryption;

// Magic bytes that identify encrypted profile files. Legacy installs
// produced `MGLA` blobs (AES-256-GCM body); current writers produce `MGL2`
// blobs (AES-256-GCM-SIV body). `load()` reads either; `save()` writes V2.
// `pub(crate)` because `services::encryption` consumes them in its test
// fixtures alongside `Encryption::decrypt_v1` / `decrypt_v2`.
pub(crate) const ENCRYPTED_MAGIC_V1: &[u8] = b"MGLA";
pub(crate) const ENCRYPTED_MAGIC_V2: &[u8] = b"MGL2";
pub(crate) const ENCRYPTED_MAGIC_LEN: usize = 4;

impl super::ProfileManager {
    /// Get all profile names from the profiles directory.
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

    /// Get summaries of all profiles (for list display). Includes services
    /// list for each profile.
    pub async fn get_all_summaries(&self) -> Result<Vec<ProfileSummary>, CoreError> {
        let names = self.get_all_names().await?;
        let order_map = self.ensure_order_indexes().await?;

        let mut summaries = Vec::new();

        for name in names {
            let is_encrypted = self.is_encrypted(&name);

            // Try to load the profile (unencrypted profiles only). Encrypted
            // profiles show minimal info since we can't read them without
            // the password.
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

    /// Load a profile by name. If password is provided, will attempt to
    /// decrypt. If no password, will try unencrypted first, and return
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

            // The file extension is the authoritative encryption flag —
            // overwrite whatever was stored inside the blob so the loaded
            // Profile can't lie about its own encryption state.
            let mut profile: Profile = serde_json::from_str(&content)?;
            profile.encrypted = true;
            // Same safety-critical PII unwrap as the plaintext branch.
            // Even for password-encrypted profiles, the blocklist
            // entries inside the blob are individually `ENC2::` wrapped
            // (machine-key, not the profile password), so unwrap here
            // unconditionally.
            self.decrypt_pii_blocklist(&mut profile)?;
            return Ok(profile);
        }

        if json_path.exists() {
            let content = std::fs::read_to_string(&json_path)?;
            let mut profile: Profile = serde_json::from_str(&content)?;
            profile.encrypted = false;
            // Safety-critical: pii_blocklist entries are written `ENC2::`
            // wrapped (`encrypt_pii_blocklist`) so the on-disk JSON
            // never holds real names / deadnames as plaintext.
            // `decrypt_pii_blocklist` must run on every load path so
            // callers see cleartext phrases — otherwise the PII filter
            // compares messages against ciphertext blobs and silently
            // lets blocked phrases through (regression caught by the
            // `chat-send-pii-blocks-message` integration case).
            // Keyed off the machine key (NOT the profile password),
            // so it's safe to run here in the no-password branch too.
            self.decrypt_pii_blocklist(&mut profile)?;
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
            if let Some(audit) = self.audit() {
                if let Err(e) = audit.record(crate::services::AuditAction::ProfileDeleted {
                    name: name.to_string(),
                }) {
                    log::error!("profile_manager failed to append ProfileDeleted audit entry: {e}");
                }
            }
            Ok(())
        } else {
            log::warn!("Profile not found for deletion: {name}");
            Err(CoreError::ProfileNotFound {
                name: name.to_string(),
            })
        }
    }

    /// Check if a profile is encrypted. Returns false for invalid profile
    /// names (fails safely).
    ///
    /// Considers a profile encrypted if EITHER a `.mgs` file exists (the
    /// expected case) OR a `.json` file starts with the `MGLA`/`MGL2`
    /// magic. The magic-byte fallback covers the corruption case where
    /// a `.mgs` file is renamed to `.json` (user mistake, backup tool
    /// that strips unknown extensions, attacker with write access). Pre-
    /// fix that file would be misclassified as plaintext, then the load
    /// path would attempt to JSON-deserialize the binary ciphertext and
    /// surface an opaque parse error instead of correctly prompting for
    /// the password.
    pub fn is_encrypted(&self, name: &str) -> bool {
        // Validate profile name to prevent path traversal attacks. For this
        // method, we return false for invalid names (fail safely).
        if validate_profile_name(name).is_err() {
            return false;
        }

        let mgs_path = self.profiles_dir.join(format!("{name}.mgs"));
        if mgs_path.exists() {
            return true;
        }
        let json_path = self.profiles_dir.join(format!("{name}.json"));
        // Read the first 4 bytes only — magic check is cheap and we
        // don't want to slurp big files just to classify them.
        if let Ok(mut f) = std::fs::File::open(&json_path) {
            use std::io::Read;
            let mut header = [0u8; ENCRYPTED_MAGIC_LEN];
            if f.read_exact(&mut header).is_ok()
                && (header == *ENCRYPTED_MAGIC_V1 || header == *ENCRYPTED_MAGIC_V2)
            {
                return true;
            }
        }
        false
    }

    /// Validate that no *other* profile claims the same RTMP
    /// `(bindAddress, port)` pair. Returns `CoreError::PortConflict` when
    /// another profile owns the same input. The current profile (matched
    /// by `profile_id`) is excluded from the scan so saving the same
    /// profile with unchanged input is a no-op.
    ///
    /// Only unencrypted profiles can be scanned without a password;
    /// encrypted profiles are skipped (we cannot determine their input
    /// config without the user's password, and the rest of the rewrite
    /// assumes the user remembers their own conflicts).
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
}
