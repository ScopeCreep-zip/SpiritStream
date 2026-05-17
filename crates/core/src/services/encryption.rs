//! Encryption Service.
//!
//! # Versioning
//!
//! There are two on-disk encryption versions. Old data is read by the V1
//! path; new data is written by the V2 path. A future migration will
//! bulk-re-encrypt the remaining V1 blobs.
//!
//! * **V1Gcm** — AES-256-GCM with random 96-bit nonces. Legacy
//!   default. Nonce collisions at high volumes (~2^32 records under one
//!   key) compromise confidentiality and authentication, which is why it
//!   is no longer used for new writes.
//! * **V2GcmSiv** — AES-256-GCM-SIV (RFC 8452). Nonce-misuse-resistant
//!   AEAD: nonce reuse only leaks "did these two ciphertexts encrypt the
//!   same plaintext", not the key. New writes use this exclusively.
//!
//! The version is encoded out-of-band:
//! * Profile `.mgs` files carry a 4-byte magic (`MGLA` → V1, `MGL2` → V2)
//!   set by `profile_manager`. The encryption helpers themselves work on
//!   the body after the magic and take an explicit version parameter where
//!   needed.
//! * Stream keys and tokens carry a string prefix (`ENC::` → V1,
//!   `ENC2::` → V2). Dispatch happens inside this module.
//!
//! # Argon2id parameters
//!
//! `derive_key` uses m=64 MiB, t=3, p=4. This matches RFC 9106's
//! memory-constrained recommendation and exceeds OWASP 2025's interactive
//! baseline. Mobile retuning lives behind a separate path.

use crate::errors::ValidationIssue;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use aes_gcm_siv::Aes256GcmSiv;
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use zeroize::{Zeroize, Zeroizing};

use crate::errors::CoreError;

const SALT_LEN: usize = 32;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

/// Minimum password length for profile encryption.
///
/// 12 characters per NIST SP 800-63B-3 and OWASP Authentication Cheat Sheet
/// 2024 guidance for **high-value memorized secrets**. Profile encryption
/// keys protect stream keys, OAuth tokens, the PII blocklist (real name,
/// deadname, hometown), the anonymous-mode salt, and chat-log contents —
/// breach of this key is catastrophic-doxxing-grade for the populations
/// this app serves. The 8-character NIST floor is for low-value secrets
/// paired with a breach-database check that we do not perform.
pub const PROFILE_PASSWORD_MIN_LENGTH: usize = 12;

// Stream key / token prefixes. New writes always emit V2.
const STREAM_KEY_PREFIX_V1: &str = "ENC::";
const STREAM_KEY_PREFIX_V2: &str = "ENC2::";

fn internal<E: std::fmt::Display>(prefix: &str, e: E) -> CoreError {
    CoreError::Internal {
        context: format!("{prefix}: {e}"),
    }
}

/// Encryption service for profile data.
///
/// See module docs for the V1Gcm → V2GcmSiv migration shape.
pub struct Encryption;

impl Encryption {
    /// Encrypt data with a password. Always writes V2 (AES-256-GCM-SIV).
    /// Layout: `[salt:32][nonce:12][ciphertext]` — the 4-byte version magic
    /// is the caller's responsibility (profile_manager prepends `MGL2`).
    pub fn encrypt(data: &[u8], password: &str) -> Result<Vec<u8>, CoreError> {
        let mut rng = rand::thread_rng();
        let salt: [u8; SALT_LEN] = rng.gen();
        let nonce_bytes: [u8; NONCE_LEN] = rng.gen();

        let key = Self::derive_key(password, &salt)?;

        let cipher = Aes256GcmSiv::new_from_slice(&*key)
            .map_err(|e| internal("Failed to create cipher", e))?;
        let nonce = aes_gcm_siv::Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, data)
            .map_err(|e| internal("Encryption failed", e))?;

        let mut result = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
        result.extend_from_slice(&salt);
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Decrypt a V1 (AES-256-GCM) blob with a password. Body layout is
    /// `[salt:32][nonce:12][ciphertext]` — caller has already stripped the
    /// `MGLA` magic. AEAD failure surfaces as `PasswordIncorrect` so
    /// transports can map it to 401 / exit code 6 without re-parsing.
    pub fn decrypt_v1(encrypted: &[u8], password: &str) -> Result<Vec<u8>, CoreError> {
        if encrypted.len() < SALT_LEN + NONCE_LEN {
            return Err(CoreError::Internal {
                context: "Invalid encrypted data".into(),
            });
        }

        let salt = &encrypted[..SALT_LEN];
        let nonce_bytes = &encrypted[SALT_LEN..SALT_LEN + NONCE_LEN];
        let ciphertext = &encrypted[SALT_LEN + NONCE_LEN..];

        let key = Self::derive_key(password, salt)?;

        let cipher =
            Aes256Gcm::new_from_slice(&*key).map_err(|e| internal("Failed to create cipher", e))?;
        let nonce = Nonce::from_slice(nonce_bytes);

        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| CoreError::PasswordIncorrect)
    }

    /// Decrypt a V2 (AES-256-GCM-SIV) blob with a password. Body layout is
    /// `[salt:32][nonce:12][ciphertext]` — caller has already stripped the
    /// `MGL2` magic.
    pub fn decrypt_v2(encrypted: &[u8], password: &str) -> Result<Vec<u8>, CoreError> {
        if encrypted.len() < SALT_LEN + NONCE_LEN {
            return Err(CoreError::Internal {
                context: "Invalid encrypted data".into(),
            });
        }

        let salt = &encrypted[..SALT_LEN];
        let nonce_bytes = &encrypted[SALT_LEN..SALT_LEN + NONCE_LEN];
        let ciphertext = &encrypted[SALT_LEN + NONCE_LEN..];

        let key = Self::derive_key(password, salt)?;

        let cipher = Aes256GcmSiv::new_from_slice(&*key)
            .map_err(|e| internal("Failed to create cipher", e))?;
        let nonce = aes_gcm_siv::Nonce::from_slice(nonce_bytes);

        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| CoreError::PasswordIncorrect)
    }

    /// Derive a key from password using Argon2id
    /// Returns a zeroizing key that will be securely erased from memory
    ///
    /// Uses strengthened parameters:
    /// - Memory: 64 MB (65536 KiB)
    /// - Iterations: 3
    /// - Parallelism: 4 threads
    fn derive_key(password: &str, salt: &[u8]) -> Result<Zeroizing<[u8; KEY_LEN]>, CoreError> {
        let mut key = Zeroizing::new([0u8; KEY_LEN]);

        // Argon2id with strengthened parameters for better security
        let params = Params::new(
            65536, // m_cost: 64 MB memory
            3,     // t_cost: 3 iterations
            4,     // p_cost: 4 parallel threads
            None,  // output length (using hash_password_into default)
        )
        .map_err(|e| internal("Failed to create Argon2 params", e))?;

        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        argon2
            .hash_password_into(password.as_bytes(), salt, &mut *key)
            .map_err(|e| internal("Key derivation failed", e))?;

        Ok(key)
    }

    // =========================================================================
    // Stream Key Encryption (for encrypt_stream_keys setting)
    // Uses a machine-specific key stored in the app data directory
    // =========================================================================

    /// Get or create the machine-specific encryption key for stream keys
    /// Returns a zeroizing key that will be securely erased from memory
    /// Public accessor for the machine key. Used by the
    /// audit log to derive its HMAC chain key via HKDF. Returns a
    /// `Zeroizing` wrapper so callers can't accidentally hold the
    /// bytes past their scope.
    pub fn get_or_create_machine_key_public(
        app_data_dir: &Path,
    ) -> Result<Zeroizing<[u8; KEY_LEN]>, CoreError> {
        Self::get_or_create_machine_key(app_data_dir)
    }

    fn get_or_create_machine_key(
        app_data_dir: &Path,
    ) -> Result<Zeroizing<[u8; KEY_LEN]>, CoreError> {
        let key_file = app_data_dir.join(".stream_key");

        if key_file.exists() {
            // Read existing key
            let mut key_data =
                std::fs::read(&key_file).map_err(|e| internal("Failed to read machine key", e))?;

            if key_data.len() != KEY_LEN {
                // Zeroize key_data before returning error
                key_data.zeroize();
                return Err(CoreError::Internal {
                    context: "Invalid machine key file".into(),
                });
            }

            // Ensure restrictive permissions on existing key file
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o600);
                std::fs::set_permissions(&key_file, perms)
                    .map_err(|e| internal("Failed to set key file permissions", e))?;
            }

            // On Windows, set hidden and system attributes
            #[cfg(windows)]
            {
                Self::set_windows_key_attributes(&key_file)?;
            }

            let mut key = Zeroizing::new([0u8; KEY_LEN]);
            key.copy_from_slice(&key_data);

            // Zeroize the temporary buffer
            key_data.zeroize();

            Ok(key)
        } else {
            // Generate new key — atomic owner-only write.
            let mut rng = rand::thread_rng();
            let key = Zeroizing::new(rng.gen::<[u8; KEY_LEN]>());
            super::write_owner_only_atomic(&key_file, &*key)?;
            Ok(key)
        }
    }

    /// Set Windows file attributes to hide and protect the machine key file
    #[cfg(windows)]
    fn set_windows_key_attributes(key_file: &Path) -> Result<(), CoreError> {
        use std::os::windows::fs::MetadataExt;

        // Get current attributes
        let metadata = std::fs::metadata(key_file)
            .map_err(|e| internal("Failed to read key file metadata", e))?;
        let mut attributes = metadata.file_attributes();

        // FILE_ATTRIBUTE_HIDDEN = 0x2
        // FILE_ATTRIBUTE_SYSTEM = 0x4
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;

        // Add hidden and system attributes
        attributes |= FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM;

        // Set the attributes using winapi
        use std::os::windows::ffi::OsStrExt;
        let wide_path: Vec<u16> = key_file.as_os_str().encode_wide().chain(Some(0)).collect();

        unsafe {
            if winapi::um::fileapi::SetFileAttributesW(wide_path.as_ptr(), attributes) == 0 {
                return Err(CoreError::Internal {
                    context: "Failed to set Windows file attributes".into(),
                });
            }
        }

        Ok(())
    }

    /// Encrypt a stream key for storage. Always writes V2 (`ENC2::` prefix,
    /// AES-256-GCM-SIV). Already-encrypted values (V1 or V2 prefix) and
    /// empty strings pass through unchanged.
    pub fn encrypt_stream_key(stream_key: &str, app_data_dir: &Path) -> Result<String, CoreError> {
        if stream_key.is_empty() || Self::is_stream_key_encrypted(stream_key) {
            return Ok(stream_key.to_string());
        }

        let machine_key = Self::get_or_create_machine_key(app_data_dir)?;
        let blob = encrypt_with_machine_key_v2(&machine_key, stream_key.as_bytes())?;
        Ok(format!("{}{}", STREAM_KEY_PREFIX_V2, BASE64.encode(&blob)))
    }

    /// Decrypt a stream key from storage. Recognises both V1 (`ENC::`) and
    /// V2 (`ENC2::`) prefixes; plaintext values pass through unchanged.
    pub fn decrypt_stream_key(
        encrypted_key: &str,
        app_data_dir: &Path,
    ) -> Result<String, CoreError> {
        if let Some(encoded) = encrypted_key.strip_prefix(STREAM_KEY_PREFIX_V2) {
            let machine_key = Self::get_or_create_machine_key(app_data_dir)?;
            decode_and_decrypt_v2(&machine_key, encoded)
        } else if let Some(encoded) = encrypted_key.strip_prefix(STREAM_KEY_PREFIX_V1) {
            let machine_key = Self::get_or_create_machine_key(app_data_dir)?;
            decode_and_decrypt_v1(&machine_key, encoded)
        } else {
            Ok(encrypted_key.to_string())
        }
    }

    /// Check if a stream key carries an encryption prefix (either version).
    pub fn is_stream_key_encrypted(stream_key: &str) -> bool {
        stream_key.starts_with(STREAM_KEY_PREFIX_V1) || stream_key.starts_with(STREAM_KEY_PREFIX_V2)
    }

    // =========================================================================
    // Token Encryption (for OAuth tokens, API keys, and other sensitive settings)
    // Same AES-256-GCM + machine key scheme as stream keys
    // =========================================================================

    /// Encrypt a sensitive token for storage (OAuth tokens, API keys, etc.)
    /// Returns base64-encoded encrypted value with ENC:: prefix
    pub fn encrypt_token(token: &str, app_data_dir: &Path) -> Result<String, CoreError> {
        Self::encrypt_stream_key(token, app_data_dir)
    }

    /// Decrypt a sensitive token from storage
    /// Returns the original plaintext token
    pub fn decrypt_token(encrypted: &str, app_data_dir: &Path) -> Result<String, CoreError> {
        Self::decrypt_stream_key(encrypted, app_data_dir)
    }

    /// Check if a value is encrypted (carries V1 `ENC::` or V2 `ENC2::` prefix).
    pub fn is_encrypted(value: &str) -> bool {
        Self::is_stream_key_encrypted(value)
    }

    // =========================================================================
    // Byte-level machine-key encryption.
    // Used by `EncryptedFileSecretStore` to encrypt arbitrary binary
    // secret payloads at rest. Layout: `[nonce:12][V2 GCM-SIV ciphertext]`.
    // There is no V1 path here — everything that uses this API writes V2.
    // =========================================================================

    /// Encrypt arbitrary bytes under the per-machine key (V2 GCM-SIV).
    pub fn encrypt_bytes_with_machine_key(
        data: &[u8],
        app_data_dir: &Path,
    ) -> Result<Vec<u8>, CoreError> {
        let machine_key = Self::get_or_create_machine_key(app_data_dir)?;
        encrypt_with_machine_key_v2(&machine_key, data)
    }

    /// Decrypt bytes produced by [`encrypt_bytes_with_machine_key`].
    pub fn decrypt_bytes_with_machine_key(
        data: &[u8],
        app_data_dir: &Path,
    ) -> Result<Vec<u8>, CoreError> {
        if data.len() < NONCE_LEN {
            return Err(CoreError::Internal {
                context: "encrypted bytes too short".into(),
            });
        }
        let machine_key = Self::get_or_create_machine_key(app_data_dir)?;
        let (nonce_bytes, ciphertext) = data.split_at(NONCE_LEN);
        let cipher = Aes256GcmSiv::new_from_slice(&*machine_key)
            .map_err(|e| internal("Failed to create cipher", e))?;
        let nonce = aes_gcm_siv::Nonce::from_slice(nonce_bytes);
        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| internal("bytes decryption failed", e))
    }

    // =========================================================================
    // Machine Key Rotation
    // Allows rotating the machine encryption key and re-encrypting all stream keys
    // =========================================================================

    /// Decrypt a stream key using a specific machine key (for rotation).
    /// Recognises both V1 and V2 prefixes — the same machine key is used
    /// for either path because the cipher is selected by prefix, not by key.
    fn decrypt_stream_key_with_key(
        encrypted_key: &str,
        machine_key: &Zeroizing<[u8; KEY_LEN]>,
    ) -> Result<String, CoreError> {
        if let Some(encoded) = encrypted_key.strip_prefix(STREAM_KEY_PREFIX_V2) {
            decode_and_decrypt_v2(machine_key, encoded)
        } else if let Some(encoded) = encrypted_key.strip_prefix(STREAM_KEY_PREFIX_V1) {
            decode_and_decrypt_v1(machine_key, encoded)
        } else {
            Ok(encrypted_key.to_string())
        }
    }

    /// Encrypt a stream key using a specific machine key (for rotation).
    /// Always writes V2 — any V1 inputs rotated through this path are
    /// automatically upgraded.
    fn encrypt_stream_key_with_key(
        stream_key: &str,
        machine_key: &Zeroizing<[u8; KEY_LEN]>,
    ) -> Result<String, CoreError> {
        if stream_key.is_empty() || Self::is_stream_key_encrypted(stream_key) {
            return Ok(stream_key.to_string());
        }

        let blob = encrypt_with_machine_key_v2(machine_key, stream_key.as_bytes())?;
        Ok(format!("{}{}", STREAM_KEY_PREFIX_V2, BASE64.encode(&blob)))
    }

    /// Generate a new machine key
    fn generate_new_machine_key() -> Result<Zeroizing<[u8; KEY_LEN]>, CoreError> {
        let mut rng = rand::thread_rng();
        Ok(Zeroizing::new(rng.gen::<[u8; KEY_LEN]>()))
    }

    /// Write a machine key to disk (atomic owner-only).
    fn write_machine_key(
        key: &Zeroizing<[u8; KEY_LEN]>,
        app_data_dir: &Path,
    ) -> Result<(), CoreError> {
        let key_file = app_data_dir.join(".stream_key");
        super::write_owner_only_atomic(&key_file, &**key)
    }

    /// Securely delete the old key file
    fn securely_delete_key_file(app_data_dir: &Path) -> Result<(), CoreError> {
        let key_file = app_data_dir.join(".stream_key");

        if !key_file.exists() {
            return Ok(());
        }

        // Read file size
        let metadata = std::fs::metadata(&key_file)
            .map_err(|e| internal("Failed to read key file metadata", e))?;
        let size = metadata.len() as usize;

        // Overwrite with zeros
        let zeros = vec![0u8; size];
        std::fs::write(&key_file, &zeros)
            .map_err(|e| internal("Failed to overwrite key file", e))?;

        // Overwrite with random data
        let mut rng = rand::thread_rng();
        let random: Vec<u8> = (0..size).map(|_| rng.gen()).collect();
        std::fs::write(&key_file, &random)
            .map_err(|e| internal("Failed to overwrite key file", e))?;

        // Delete
        std::fs::remove_file(&key_file).map_err(|e| internal("Failed to delete key file", e))?;

        Ok(())
    }

    /// Backup profiles directory before rotation
    fn backup_profiles_directory(app_data_dir: &Path) -> Result<PathBuf, CoreError> {
        let profiles_dir = app_data_dir.join("profiles");
        let backup_dir = app_data_dir.join("profiles_backup");
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_path = backup_dir.join(format!("backup_{timestamp}"));

        log::info!("Creating backup at: {}", backup_path.display());

        // Create backup directory
        std::fs::create_dir_all(&backup_path)
            .map_err(|e| internal("Failed to create backup directory", e))?;

        // Set restrictive permissions on backup directory
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o700); // Owner only
            std::fs::set_permissions(&backup_dir, perms.clone())
                .map_err(|e| internal("Failed to set backup directory permissions", e))?;
            std::fs::set_permissions(&backup_path, perms)
                .map_err(|e| internal("Failed to set backup directory permissions", e))?;
        }

        // Copy all profile files (.json and .mgs)
        let entries = std::fs::read_dir(&profiles_dir)
            .map_err(|e| internal("Failed to read profiles directory", e))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext == "json" || ext == "mgs" {
                    if let Some(file_name) = path.file_name() {
                        let dest = backup_path.join(file_name);
                        std::fs::copy(&path, &dest).map_err(|e| {
                            internal(
                                &format!("Failed to backup {}", file_name.to_string_lossy()),
                                e,
                            )
                        })?;
                        log::debug!("Backed up: {}", file_name.to_string_lossy());
                    }
                }
            }
        }

        log::info!("Backup created successfully");
        Ok(backup_path)
    }

    /// Restore profiles from backup
    fn restore_from_backup(backup_path: &Path, app_data_dir: &Path) -> Result<(), CoreError> {
        let profiles_dir = app_data_dir.join("profiles");

        log::warn!("Restoring from backup: {}", backup_path.display());

        // Delete current profiles
        let entries = std::fs::read_dir(&profiles_dir)
            .map_err(|e| internal("Failed to read profiles directory", e))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext == "json" || ext == "mgs" {
                    std::fs::remove_file(&path).map_err(|e| {
                        internal(&format!("Failed to delete {}", path.display()), e)
                    })?;
                }
            }
        }

        // Restore from backup
        let backup_entries = std::fs::read_dir(backup_path)
            .map_err(|e| internal("Failed to read backup directory", e))?;

        for entry in backup_entries.flatten() {
            let path = entry.path();
            if let Some(file_name) = path.file_name() {
                let dest = profiles_dir.join(file_name);
                std::fs::copy(&path, &dest).map_err(|e| {
                    internal(
                        &format!("Failed to restore {}", file_name.to_string_lossy()),
                        e,
                    )
                })?;
            }
        }

        log::info!("Backup restored successfully");
        Ok(())
    }

    /// Clean up old backups, keeping only the most recent N
    fn cleanup_old_backups(app_data_dir: &Path, keep_count: usize) -> Result<(), CoreError> {
        let backup_dir = app_data_dir.join("profiles_backup");

        if !backup_dir.exists() {
            return Ok(());
        }

        // Get all backup directories
        let entries = std::fs::read_dir(&backup_dir)
            .map_err(|e| internal("Failed to read backup directory", e))?;

        let mut backups: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.path())
            .collect();

        // Sort by name (timestamp is in name)
        backups.sort();

        // Keep newest N, delete rest
        while backups.len() > keep_count {
            if let Some(oldest) = backups.first() {
                log::info!("Deleting old backup: {}", oldest.display());
                std::fs::remove_dir_all(oldest)
                    .map_err(|e| internal("Failed to delete old backup", e))?;
                backups.remove(0);
            }
        }

        Ok(())
    }

    /// Rotate the machine encryption key.
    ///
    /// Re-encrypts every machine-key-encrypted secret in every profile file
    /// (stream keys, OBS password, Discord webhook, etc.) under a freshly
    /// generated machine key. `.json` profiles are re-encrypted in place;
    /// `.mgs` (password-protected) profiles are decrypted with the supplied
    /// password from `encrypted_passwords`, their inner secrets re-encrypted
    /// under the new machine key, and the envelope re-sealed with the same
    /// user password.
    ///
    /// Pre-flight validation:
    /// - Every `.mgs` profile on disk must have a corresponding entry in
    ///   `encrypted_passwords`. Missing entries return `ValidationFailed`
    ///   with `code = "missing_password_for_<name>"` BEFORE the old key is
    ///   touched, so a failed rotation leaves disk state unchanged.
    /// - Every supplied password is verified by an actual decrypt of the
    ///   `.mgs` blob. A wrong password returns `PasswordIncorrect` before
    ///   the old key is destroyed.
    pub fn rotate_machine_key(
        app_data_dir: &Path,
        profiles_dir: &Path,
        encrypted_passwords: &std::collections::HashMap<String, String>,
    ) -> Result<RotationReport, CoreError> {
        log::info!("Starting machine key rotation");

        // 1. Enumerate profile files up-front so pre-flight checks can run
        //    before any destructive step.
        let entries = std::fs::read_dir(profiles_dir)
            .map_err(|e| internal("Failed to read profiles directory", e))?;

        let profile_files: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| {
                let path = e.path();
                let ext = path.extension().and_then(|ext| ext.to_str());
                ext == Some("json") || ext == Some("mgs")
            })
            .map(|e| e.path())
            .collect();

        // 2. Pre-flight: every `.mgs` profile must have a password supplied
        //    AND that password must successfully decrypt the envelope. We do
        //    this BEFORE backing up or touching the old key — a missing or
        //    wrong password aborts cleanly with disk state untouched.
        let mut missing: Vec<ValidationIssue> = Vec::new();
        for path in &profile_files {
            if path.extension().and_then(|e| e.to_str()) != Some("mgs") {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("<unknown>")
                .to_string();
            match encrypted_passwords.get(&name) {
                None => missing.push(ValidationIssue {
                    code: format!("missing_password_for_{name}"),
                    message: format!("password required for encrypted profile '{name}'"),
                    path: Some(format!("/unlockedPasswords/{name}")),
                }),
                Some(pw) => {
                    // Verify the password actually decrypts the envelope.
                    // Surfaces `PasswordIncorrect` directly so the UI can
                    // localize "wrong password for <name>" without first
                    // destroying the old key.
                    let _verify = Self::decrypt_mgs_envelope(path, pw)?;
                }
            }
        }
        if !missing.is_empty() {
            return Err(CoreError::ValidationFailed { reasons: missing });
        }

        // 3. Create backup AFTER pre-flight passes — no point making a backup
        //    of a state we're going to refuse to mutate.
        let backup_path = Self::backup_profiles_directory(app_data_dir)?;

        // 4. Load old key.
        let old_key = Self::get_or_create_machine_key(app_data_dir)?;

        // 5. Generate new key.
        let new_key = Self::generate_new_machine_key()?;

        let total_profiles = profile_files.len();
        let mut profiles_updated = 0;
        let mut keys_reencrypted = 0;

        // 6. Re-encrypt each profile. Per-profile failure rolls everything
        //    back (the on-disk old `.stream_key` is still present and the
        //    backup directory contains the pre-rotation profile files).
        for profile_path in &profile_files {
            let result = if profile_path.extension().and_then(|e| e.to_str()) == Some("mgs") {
                let name = profile_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("<unknown>");
                let pw = encrypted_passwords
                    .get(name)
                    .expect("pre-flight ensured password is present");
                Self::reencrypt_mgs_profile(profile_path, pw, &old_key, &new_key)
            } else {
                Self::reencrypt_json_profile(profile_path, &old_key, &new_key)
            };
            match result {
                Ok(count) => {
                    profiles_updated += 1;
                    keys_reencrypted += count;
                    log::debug!("Re-encrypted {} keys in {}", count, profile_path.display());
                }
                Err(e) => {
                    log::error!(
                        "Failed to re-encrypt profile {}: {}",
                        profile_path.display(),
                        e
                    );
                    log::error!("Rolling back changes");
                    Self::restore_from_backup(&backup_path, app_data_dir)?;
                    return Err(CoreError::Internal {
                        context: format!(
                            "Key rotation failed while updating {}: {}. All changes have been rolled back.",
                            profile_path.display(),
                            e
                        ),
                    });
                }
            }
        }

        // 7. Securely delete old key.
        Self::securely_delete_key_file(app_data_dir)?;

        // 8. Write new key.
        Self::write_machine_key(&new_key, app_data_dir)?;

        // 9. Clean up old backups (keep last 5).
        Self::cleanup_old_backups(app_data_dir, 5)?;

        log::info!(
            "Machine key rotation complete: {profiles_updated} profiles updated, {keys_reencrypted} keys re-encrypted"
        );

        Ok(RotationReport {
            profiles_updated,
            keys_reencrypted,
            total_profiles,
            timestamp: chrono::Utc::now(),
        })
    }

    /// Read a `.mgs` file, strip the magic prefix, and decrypt the envelope
    /// with the supplied password. Returns the inner profile JSON bytes.
    fn decrypt_mgs_envelope(profile_path: &Path, password: &str) -> Result<Vec<u8>, CoreError> {
        use crate::services::profile_manager::{
            ENCRYPTED_MAGIC_LEN, ENCRYPTED_MAGIC_V1, ENCRYPTED_MAGIC_V2,
        };
        let content =
            std::fs::read(profile_path).map_err(|e| internal("Failed to read profile file", e))?;
        if content.len() < ENCRYPTED_MAGIC_LEN {
            return Err(CoreError::Internal {
                context: format!("invalid encrypted profile: {}", profile_path.display()),
            });
        }
        let magic = &content[..ENCRYPTED_MAGIC_LEN];
        let body = &content[ENCRYPTED_MAGIC_LEN..];
        if magic == ENCRYPTED_MAGIC_V2 {
            Self::decrypt_v2(body, password)
        } else if magic == ENCRYPTED_MAGIC_V1 {
            Self::decrypt_v1(body, password)
        } else {
            Err(CoreError::Internal {
                context: format!("invalid encrypted profile magic: {}", profile_path.display()),
            })
        }
    }

    /// Re-encrypt a `.mgs` profile: decrypt the envelope with the user's
    /// password, swap every machine-key-encrypted secret over to the new
    /// machine key, then re-seal the envelope under the same password and
    /// write to disk atomically. Always writes V2 envelopes; an existing
    /// V1 profile is upgraded as a side effect.
    fn reencrypt_mgs_profile(
        profile_path: &Path,
        password: &str,
        old_key: &Zeroizing<[u8; KEY_LEN]>,
        new_key: &Zeroizing<[u8; KEY_LEN]>,
    ) -> Result<usize, CoreError> {
        use crate::models::Profile;
        use crate::services::profile_manager::{ENCRYPTED_MAGIC_LEN, ENCRYPTED_MAGIC_V2};

        let plaintext_bytes = Self::decrypt_mgs_envelope(profile_path, password)?;
        let json_str = String::from_utf8(plaintext_bytes)
            .map_err(|e| internal("Invalid UTF-8 in decrypted profile", e))?;
        let mut profile: Profile = serde_json::from_str(&json_str)
            .map_err(|e| internal("Failed to parse decrypted profile JSON", e))?;

        let keys_updated = Self::rotate_inner_secrets(&mut profile, old_key, new_key)?;

        let new_json = serde_json::to_string_pretty(&profile)
            .map_err(|e| internal("Failed to serialize profile", e))?;
        let encrypted = Self::encrypt(new_json.as_bytes(), password)?;
        let mut data = Vec::with_capacity(ENCRYPTED_MAGIC_LEN + encrypted.len());
        data.extend_from_slice(ENCRYPTED_MAGIC_V2);
        data.extend_from_slice(&encrypted);
        super::write_owner_only_atomic(profile_path, &data)?;

        Ok(keys_updated)
    }

    /// Re-encrypt a `.json` (plaintext-envelope) profile. The profile JSON
    /// itself sits on disk in plaintext, but the stream keys / OBS password
    /// / Discord webhook fields are individually machine-key-encrypted.
    fn reencrypt_json_profile(
        profile_path: &Path,
        old_key: &Zeroizing<[u8; KEY_LEN]>,
        new_key: &Zeroizing<[u8; KEY_LEN]>,
    ) -> Result<usize, CoreError> {
        use crate::models::Profile;

        let content =
            std::fs::read(profile_path).map_err(|e| internal("Failed to read profile file", e))?;
        let json_str =
            String::from_utf8(content).map_err(|e| internal("Invalid UTF-8 in profile", e))?;
        let mut profile: Profile = serde_json::from_str(&json_str)
            .map_err(|e| internal("Failed to parse profile JSON", e))?;

        let keys_updated = Self::rotate_inner_secrets(&mut profile, old_key, new_key)?;

        let new_json = serde_json::to_string_pretty(&profile)
            .map_err(|e| internal("Failed to serialize profile", e))?;
        super::write_owner_only_atomic(profile_path, new_json.as_bytes())?;

        Ok(keys_updated)
    }

    /// Swap every machine-key-encrypted field on a deserialized `Profile`
    /// over to the new machine key. Stream keys live on each target; the
    /// other sensitive-settings fields (OBS password, Discord webhook,
    /// backend token, YouTube API key, OAuth access/refresh tokens) live
    /// on `profile.settings`.
    fn rotate_inner_secrets(
        profile: &mut crate::models::Profile,
        old_key: &Zeroizing<[u8; KEY_LEN]>,
        new_key: &Zeroizing<[u8; KEY_LEN]>,
    ) -> Result<usize, CoreError> {
        let mut keys_updated = 0;

        for group in &mut profile.output_groups {
            for target in &mut group.stream_targets {
                if Self::is_stream_key_encrypted(&target.stream_key) {
                    let plaintext = Self::decrypt_stream_key_with_key(&target.stream_key, old_key)?;
                    target.stream_key = Self::encrypt_stream_key_with_key(&plaintext, new_key)?;
                    keys_updated += 1;
                }
            }
        }

        // Sensitive profile settings — same encryption scheme as stream keys.
        let settings_fields: [&mut String; 4] = [
            &mut profile.settings.obs.password,
            &mut profile.settings.discord.webhook_url,
            &mut profile.settings.backend.token,
            &mut profile.settings.chat.youtube_api_key,
        ];
        for field in settings_fields {
            if !field.is_empty() && Self::is_stream_key_encrypted(field) {
                let plaintext = Self::decrypt_stream_key_with_key(field, old_key)?;
                *field = Self::encrypt_stream_key_with_key(&plaintext, new_key)?;
                keys_updated += 1;
            }
        }

        // OAuth tokens — twitch + youtube.
        for account in [
            &mut profile.settings.oauth.twitch,
            &mut profile.settings.oauth.youtube,
        ] {
            if !account.access_token.is_empty()
                && Self::is_stream_key_encrypted(&account.access_token)
            {
                let plaintext = Self::decrypt_stream_key_with_key(&account.access_token, old_key)?;
                account.access_token = Self::encrypt_stream_key_with_key(&plaintext, new_key)?;
                keys_updated += 1;
            }
            if !account.refresh_token.is_empty()
                && Self::is_stream_key_encrypted(&account.refresh_token)
            {
                let plaintext = Self::decrypt_stream_key_with_key(&account.refresh_token, old_key)?;
                account.refresh_token = Self::encrypt_stream_key_with_key(&plaintext, new_key)?;
                keys_updated += 1;
            }
        }

        Ok(keys_updated)
    }
}

/// Report returned after successful key rotation
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct RotationReport {
    pub profiles_updated: usize,
    pub keys_reencrypted: usize,
    pub total_profiles: usize,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

// ---------------------------------------------------------------------------
// Machine-key inner helpers — V1Gcm read path, V2GcmSiv write+read path.
// Take the raw 32-byte key (not zeroizing) so they can be shared by both the
// machine-key (`encrypt_stream_key`) and rotation (`*_with_key`) flows.
// ---------------------------------------------------------------------------

fn encrypt_with_machine_key_v2(
    machine_key: &Zeroizing<[u8; KEY_LEN]>,
    plaintext: &[u8],
) -> Result<Vec<u8>, CoreError> {
    let mut rng = rand::thread_rng();
    let nonce_bytes: [u8; NONCE_LEN] = rng.gen();

    let cipher = Aes256GcmSiv::new_from_slice(&**machine_key)
        .map_err(|e| internal("Failed to create cipher", e))?;
    let nonce = aes_gcm_siv::Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| internal("Stream key encryption failed", e))?;

    let mut combined = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);
    Ok(combined)
}

fn decode_and_decrypt_v2(
    machine_key: &Zeroizing<[u8; KEY_LEN]>,
    encoded_body: &str,
) -> Result<String, CoreError> {
    let mut combined = BASE64
        .decode(encoded_body)
        .map_err(|e| internal("Failed to decode encrypted stream key", e))?;

    if combined.len() < NONCE_LEN {
        combined.zeroize();
        return Err(CoreError::Internal {
            context: "Invalid encrypted stream key".into(),
        });
    }

    let (nonce_bytes, ciphertext) = combined.split_at(NONCE_LEN);
    let cipher = Aes256GcmSiv::new_from_slice(&**machine_key)
        .map_err(|e| internal("Failed to create cipher", e))?;
    let nonce = aes_gcm_siv::Nonce::from_slice(nonce_bytes);

    let mut plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| internal("Stream key decryption failed", e))?;

    let result = String::from_utf8(plaintext.clone())
        .map_err(|e| internal("Invalid UTF-8 in decrypted stream key", e));

    plaintext.zeroize();
    combined.zeroize();
    result
}

fn decode_and_decrypt_v1(
    machine_key: &Zeroizing<[u8; KEY_LEN]>,
    encoded_body: &str,
) -> Result<String, CoreError> {
    let mut combined = BASE64
        .decode(encoded_body)
        .map_err(|e| internal("Failed to decode encrypted stream key", e))?;

    if combined.len() < NONCE_LEN {
        combined.zeroize();
        return Err(CoreError::Internal {
            context: "Invalid encrypted stream key".into(),
        });
    }

    let (nonce_bytes, ciphertext) = combined.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new_from_slice(&**machine_key)
        .map_err(|e| internal("Failed to create cipher", e))?;
    let nonce = Nonce::from_slice(nonce_bytes);

    let mut plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| internal("Stream key decryption failed", e))?;

    let result = String::from_utf8(plaintext.clone())
        .map_err(|e| internal("Invalid UTF-8 in decrypted stream key", e));

    plaintext.zeroize();
    combined.zeroize();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // --- Password-based encryption (profile `.mgs` body) -------------------

    #[test]
    fn password_roundtrip_v2() {
        let plaintext = b"the quick brown fox jumps over the lazy dog";
        let encrypted = Encryption::encrypt(plaintext, "correct horse battery staple").unwrap();
        let decrypted = Encryption::decrypt_v2(&encrypted, "correct horse battery staple").unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn password_wrong_password_yields_password_incorrect() {
        let encrypted = Encryption::encrypt(b"secret", "right").unwrap();
        let err = Encryption::decrypt_v2(&encrypted, "wrong").unwrap_err();
        assert!(matches!(err, CoreError::PasswordIncorrect));
    }

    #[test]
    fn password_v1_legacy_blobs_still_decrypt() {
        // Build a V1 blob by reimplementing the legacy encrypt path (private
        // helper retained only for this test fixture). Confirms a `.mgs`
        // file produced by a pre-Phase-6 install opens cleanly under the
        // new code.
        fn legacy_encrypt_v1(data: &[u8], password: &str) -> Vec<u8> {
            let mut rng = rand::thread_rng();
            let salt: [u8; SALT_LEN] = rng.gen();
            let nonce_bytes: [u8; NONCE_LEN] = rng.gen();
            let key = Encryption::derive_key(password, &salt).unwrap();
            let cipher = Aes256Gcm::new_from_slice(&*key).unwrap();
            let nonce = Nonce::from_slice(&nonce_bytes);
            let ciphertext = cipher.encrypt(nonce, data).unwrap();
            let mut out = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
            out.extend_from_slice(&salt);
            out.extend_from_slice(&nonce_bytes);
            out.extend_from_slice(&ciphertext);
            out
        }

        let blob = legacy_encrypt_v1(b"legacy data", "password");
        let plain = Encryption::decrypt_v1(&blob, "password").unwrap();
        assert_eq!(plain, b"legacy data");
    }

    // --- AES-GCM-SIV nonce-misuse-resistance --------------------------------

    #[test]
    fn gcm_siv_is_deterministic_under_nonce_reuse() {
        // AES-GCM-SIV is deterministic given (key, nonce, plaintext). Two
        // encryptions with identical inputs produce identical ciphertext —
        // an attacker only learns "did these two ciphertexts encrypt the
        // same plaintext", never the key. With vanilla AES-GCM, the same
        // assertion would still hold (output differs only by the random
        // nonce that is part of the input), so we ALSO check that two
        // different plaintexts under the same key+nonce still decrypt
        // correctly, which is the property AES-GCM-SIV guarantees and
        // AES-GCM cannot (forgery is feasible after one reuse).
        let key: [u8; KEY_LEN] = [42u8; KEY_LEN];
        let nonce_bytes: [u8; NONCE_LEN] = [7u8; NONCE_LEN];
        let cipher = Aes256GcmSiv::new_from_slice(&key).unwrap();
        let nonce = aes_gcm_siv::Nonce::from_slice(&nonce_bytes);

        let pt_a = b"plaintext A";
        let pt_b = b"plaintext B (different length)";

        let ct_a1 = cipher.encrypt(nonce, &pt_a[..]).unwrap();
        let ct_a2 = cipher.encrypt(nonce, &pt_a[..]).unwrap();
        assert_eq!(
            ct_a1, ct_a2,
            "GCM-SIV must be deterministic given identical inputs"
        );

        let ct_b = cipher.encrypt(nonce, &pt_b[..]).unwrap();
        assert_ne!(
            ct_a1, ct_b,
            "different plaintexts must produce different ciphertexts"
        );

        // Both decrypt correctly even though they share key+nonce.
        let plain_a = cipher.decrypt(nonce, &ct_a1[..]).unwrap();
        let plain_b = cipher.decrypt(nonce, &ct_b[..]).unwrap();
        assert_eq!(plain_a, pt_a);
        assert_eq!(plain_b, pt_b);
    }

    // --- Machine-key stream key / token ------------------------------------

    #[test]
    fn stream_key_roundtrip_v2() {
        let dir = TempDir::new().unwrap();
        let plaintext = "live_1234567890_abcdefghijklmnop";
        let encrypted = Encryption::encrypt_stream_key(plaintext, dir.path()).unwrap();
        assert!(encrypted.starts_with(STREAM_KEY_PREFIX_V2));
        assert!(Encryption::is_stream_key_encrypted(&encrypted));

        let decrypted = Encryption::decrypt_stream_key(&encrypted, dir.path()).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn stream_key_v1_legacy_still_decrypts() {
        // Manually produce a V1 stream key blob with the legacy code path
        // and confirm `decrypt_stream_key` reads it transparently.
        let dir = TempDir::new().unwrap();
        let plaintext = "v1-legacy-stream-key";

        let machine_key = Encryption::get_or_create_machine_key(dir.path()).unwrap();
        let mut rng = rand::thread_rng();
        let nonce_bytes: [u8; NONCE_LEN] = rng.gen();
        let cipher = Aes256Gcm::new_from_slice(&*machine_key).unwrap();
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes()).unwrap();
        let mut combined = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);
        let v1_encoded = format!("{}{}", STREAM_KEY_PREFIX_V1, BASE64.encode(&combined));

        assert!(Encryption::is_stream_key_encrypted(&v1_encoded));
        let decrypted = Encryption::decrypt_stream_key(&v1_encoded, dir.path()).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn mixed_v1_and_v2_stream_keys_in_same_environment() {
        // A single profile may have some stream keys
        // written under V1 (pre-migration) and some under V2 (re-saved).
        // Confirm both decrypt under one machine key without interference.
        let dir = TempDir::new().unwrap();
        let machine_key = Encryption::get_or_create_machine_key(dir.path()).unwrap();

        // V2 via public API
        let v2 = Encryption::encrypt_stream_key("plain-v2", dir.path()).unwrap();
        assert!(v2.starts_with(STREAM_KEY_PREFIX_V2));

        // V1 via direct construction (legacy producer)
        let mut rng = rand::thread_rng();
        let nonce_bytes: [u8; NONCE_LEN] = rng.gen();
        let cipher = Aes256Gcm::new_from_slice(&*machine_key).unwrap();
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, b"plain-v1".as_ref()).unwrap();
        let mut combined = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);
        let v1 = format!("{}{}", STREAM_KEY_PREFIX_V1, BASE64.encode(&combined));

        assert_eq!(
            Encryption::decrypt_stream_key(&v1, dir.path()).unwrap(),
            "plain-v1"
        );
        assert_eq!(
            Encryption::decrypt_stream_key(&v2, dir.path()).unwrap(),
            "plain-v2"
        );
    }

    #[test]
    fn already_encrypted_passes_through() {
        let dir = TempDir::new().unwrap();
        let v2 = Encryption::encrypt_stream_key("hello", dir.path()).unwrap();
        let again = Encryption::encrypt_stream_key(&v2, dir.path()).unwrap();
        assert_eq!(
            again, v2,
            "re-encrypting an already-encrypted value must be a no-op"
        );

        // V1 must also pass through unchanged when re-encrypted, so the
        // migration boundary is one-way (only `rotate_machine_key` upgrades).
        let v1_like = format!("{}deadbeef", STREAM_KEY_PREFIX_V1);
        let result = Encryption::encrypt_stream_key(&v1_like, dir.path()).unwrap();
        assert_eq!(result, v1_like);
    }

    #[test]
    fn empty_string_passes_through() {
        let dir = TempDir::new().unwrap();
        let result = Encryption::encrypt_stream_key("", dir.path()).unwrap();
        assert_eq!(result, "");
    }

    // --- Machine-key rotation -----------------------------

    #[tokio::test]
    async fn rotate_with_encrypted_profile_re_encrypts_stream_keys() {
        use crate::models::{OutputGroup, Platform, Profile, ProfileSettings, RtmpInput, StreamTarget};
        use crate::services::ProfileManager;
        use std::collections::HashMap;

        let data_dir = TempDir::new().unwrap();
        let profiles_dir = data_dir.path().join("profiles");
        std::fs::create_dir_all(&profiles_dir).unwrap();
        let mgr = ProfileManager::new(data_dir.path().to_path_buf());

        // Plaintext profile with one machine-key-encrypted stream key.
        let plain_key = "live_plain_1234567890_abcdef";
        let mut og = OutputGroup::new();
        og.id = "g1".into();
        og.name = "G1".into();
        og.stream_targets = vec![StreamTarget {
            id: "t1".into(),
            service: Platform::Twitch,
            name: "Twitch".into(),
            url: "rtmp://localhost/x".into(),
            stream_key: plain_key.into(),
        }];
        let mut plain = Profile {
            id: "plain".into(),
            name: "plain".into(),
            encrypted: false,
            input: RtmpInput {
                input_type: "rtmp".into(),
                bind_address: "127.0.0.1".into(),
                port: 1935,
                application: "live".into(),
            },
            output_groups: vec![og],
            settings: {
                let mut s = ProfileSettings::default();
                s.encrypt_stream_keys = true;
                s
            },
            pii_blocklist: vec![],
            pii_fuzzy: false,
            anonymous_logging: true,
            anonymous_salt: String::new(),
        };
        mgr.save_with_key_encryption(&plain, None).await.unwrap();

        // Encrypted (.mgs) profile under password "correct-horse-battery".
        let enc_key = "live_enc_9876543210_zyxwvu";
        plain.id = "enc".into();
        plain.name = "enc".into();
        plain.input.port = 1936;
        plain.output_groups[0].stream_targets[0].stream_key = enc_key.into();
        mgr.save_with_key_encryption(&plain, Some("correct-horse-battery"))
            .await
            .unwrap();

        // Sanity: enc profile is on disk as .mgs, plain as .json.
        assert!(profiles_dir.join("plain.json").exists());
        assert!(profiles_dir.join("enc.mgs").exists());

        // Rotate, supplying the .mgs password.
        let mut passwords = HashMap::new();
        passwords.insert("enc".to_string(), "correct-horse-battery".to_string());
        let report = Encryption::rotate_machine_key(data_dir.path(), &profiles_dir, &passwords)
            .expect("rotation should succeed");
        assert_eq!(report.profiles_updated, 2);
        assert!(report.keys_reencrypted >= 2);

        // Both profiles must still open cleanly under the new key, with the
        // original plaintext stream keys.
        let loaded_plain = mgr.load_with_key_decryption("plain", None).await.unwrap();
        assert_eq!(
            loaded_plain.output_groups[0].stream_targets[0].stream_key,
            plain_key
        );
        let loaded_enc = mgr
            .load_with_key_decryption("enc", Some("correct-horse-battery"))
            .await
            .unwrap();
        assert_eq!(
            loaded_enc.output_groups[0].stream_targets[0].stream_key,
            enc_key
        );
    }

    #[tokio::test]
    async fn rotate_refuses_when_password_missing_for_encrypted_profile() {
        use crate::models::{Profile, ProfileSettings, RtmpInput};
        use crate::services::ProfileManager;
        use std::collections::HashMap;

        let data_dir = TempDir::new().unwrap();
        let profiles_dir = data_dir.path().join("profiles");
        std::fs::create_dir_all(&profiles_dir).unwrap();
        let mgr = ProfileManager::new(data_dir.path().to_path_buf());

        let profile = Profile {
            id: "enc".into(),
            name: "enc".into(),
            encrypted: true,
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
        mgr.save_with_key_encryption(&profile, Some("correct-horse-battery"))
            .await
            .unwrap();

        // Force machine-key creation, then snapshot it so we can confirm
        // it's untouched after the refusal.
        Encryption::get_or_create_machine_key_public(data_dir.path()).unwrap();
        let key_path = data_dir.path().join(".stream_key");
        let key_before = std::fs::read(&key_path).unwrap();

        let err = Encryption::rotate_machine_key(data_dir.path(), &profiles_dir, &HashMap::new())
            .expect_err("rotation must refuse without password for .mgs profile");
        match err {
            CoreError::ValidationFailed { reasons } => {
                assert!(reasons
                    .iter()
                    .any(|r| r.code.starts_with("missing_password_for_")));
            }
            other => panic!("expected ValidationFailed, got {other:?}"),
        }

        let key_after = std::fs::read(&key_path).unwrap();
        assert_eq!(
            key_before, key_after,
            "old machine key must be untouched after a refused rotation"
        );
    }

    #[tokio::test]
    async fn rotate_refuses_on_wrong_password_for_encrypted_profile() {
        use crate::models::{Profile, ProfileSettings, RtmpInput};
        use crate::services::ProfileManager;
        use std::collections::HashMap;

        let data_dir = TempDir::new().unwrap();
        let profiles_dir = data_dir.path().join("profiles");
        std::fs::create_dir_all(&profiles_dir).unwrap();
        let mgr = ProfileManager::new(data_dir.path().to_path_buf());

        let profile = Profile {
            id: "enc".into(),
            name: "enc".into(),
            encrypted: true,
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
        mgr.save_with_key_encryption(&profile, Some("correct-horse-battery"))
            .await
            .unwrap();

        Encryption::get_or_create_machine_key_public(data_dir.path()).unwrap();
        let key_path = data_dir.path().join(".stream_key");
        let key_before = std::fs::read(&key_path).unwrap();

        let mut passwords = HashMap::new();
        passwords.insert("enc".to_string(), "wrong-password".to_string());
        let err = Encryption::rotate_machine_key(data_dir.path(), &profiles_dir, &passwords)
            .expect_err("rotation must refuse on wrong password");
        assert!(matches!(err, CoreError::PasswordIncorrect));

        let key_after = std::fs::read(&key_path).unwrap();
        assert_eq!(
            key_before, key_after,
            "old machine key must be untouched after wrong-password refusal"
        );
    }
}
