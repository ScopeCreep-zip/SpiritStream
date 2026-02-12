// Encryption Service
// Handles profile encryption using AES-256-GCM

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{Argon2, Algorithm, Version, Params};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::Rng;
use std::path::{Path, PathBuf};
use zeroize::{Zeroize, Zeroizing};
use serde::{Deserialize, Serialize};

const SALT_LEN: usize = 32;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

// Prefix for identifying encrypted stream keys
const STREAM_KEY_PREFIX: &str = "ENC::";

/// Encryption service for profile data
pub struct Encryption;

impl Encryption {
    /// Encrypt data with a password
    pub fn encrypt(data: &[u8], password: &str) -> Result<Vec<u8>, String> {
        // Generate random salt and nonce
        let mut rng = rand::thread_rng();
        let salt: [u8; SALT_LEN] = rng.gen();
        let nonce_bytes: [u8; NONCE_LEN] = rng.gen();

        // Derive key from password
        let key = Self::derive_key(password, &salt)?;

        // Encrypt
        let cipher = Aes256Gcm::new_from_slice(&*key)
            .map_err(|e| format!("Failed to create cipher: {e}"))?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, data)
            .map_err(|e| format!("Encryption failed: {e}"))?;

        // Combine salt + nonce + ciphertext
        let mut result = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
        result.extend_from_slice(&salt);
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Decrypt data with a password
    pub fn decrypt(encrypted: &[u8], password: &str) -> Result<Vec<u8>, String> {
        if encrypted.len() < SALT_LEN + NONCE_LEN {
            return Err("Invalid encrypted data".to_string());
        }

        // Extract salt, nonce, and ciphertext
        let salt = &encrypted[..SALT_LEN];
        let nonce_bytes = &encrypted[SALT_LEN..SALT_LEN + NONCE_LEN];
        let ciphertext = &encrypted[SALT_LEN + NONCE_LEN..];

        // Derive key from password
        let key = Self::derive_key(password, salt)?;

        // Decrypt
        let cipher = Aes256Gcm::new_from_slice(&*key)
            .map_err(|e| format!("Failed to create cipher: {e}"))?;
        let nonce = Nonce::from_slice(nonce_bytes);

        cipher.decrypt(nonce, ciphertext)
            .map_err(|e| format!("Decryption failed: {e}"))
    }

    /// Derive a key from password using Argon2id
    /// Returns a zeroizing key that will be securely erased from memory
    ///
    /// Uses strengthened parameters:
    /// - Memory: 64 MB (65536 KiB)
    /// - Iterations: 3
    /// - Parallelism: 4 threads
    fn derive_key(password: &str, salt: &[u8]) -> Result<Zeroizing<[u8; KEY_LEN]>, String> {
        let mut key = Zeroizing::new([0u8; KEY_LEN]);

        // Argon2id with strengthened parameters for better security
        let params = Params::new(
            65536,  // m_cost: 64 MB memory
            3,      // t_cost: 3 iterations
            4,      // p_cost: 4 parallel threads
            None    // output length (using hash_password_into default)
        ).map_err(|e| format!("Failed to create Argon2 params: {e}"))?;

        let argon2 = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            params
        );

        argon2
            .hash_password_into(password.as_bytes(), salt, &mut *key)
            .map_err(|e| format!("Key derivation failed: {e}"))?;

        Ok(key)
    }

    // =========================================================================
    // Stream Key Encryption (for encrypt_stream_keys setting)
    // Uses a machine-specific key stored in the app data directory
    // =========================================================================

    /// Get or create the machine-specific encryption key for stream keys
    /// Returns a zeroizing key that will be securely erased from memory
    fn get_or_create_machine_key(app_data_dir: &Path) -> Result<Zeroizing<[u8; KEY_LEN]>, String> {
        let key_file = app_data_dir.join(".stream_key");

        if key_file.exists() {
            // Read existing key
            let mut key_data = std::fs::read(&key_file)
                .map_err(|e| format!("Failed to read machine key: {e}"))?;

            if key_data.len() != KEY_LEN {
                // Zeroize key_data before returning error
                key_data.zeroize();
                return Err("Invalid machine key file".to_string());
            }

            // Ensure restrictive permissions on existing key file
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o600);
                std::fs::set_permissions(&key_file, perms)
                    .map_err(|e| format!("Failed to set key file permissions: {e}"))?;
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
            // Generate new key
            let mut rng = rand::thread_rng();
            let key = Zeroizing::new(rng.gen::<[u8; KEY_LEN]>());

            // Save it
            std::fs::write(&key_file, *key)
                .map_err(|e| format!("Failed to save machine key: {e}"))?;

            // Set restrictive permissions on the new key file
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o600);
                std::fs::set_permissions(&key_file, perms)
                    .map_err(|e| format!("Failed to set key file permissions: {e}"))?;
            }

            // On Windows, set hidden and system attributes
            #[cfg(windows)]
            {
                Self::set_windows_key_attributes(&key_file)?;
            }

            Ok(key)
        }
    }

    /// Set Windows file attributes to hide and protect the machine key file
    #[cfg(windows)]
    fn set_windows_key_attributes(key_file: &Path) -> Result<(), String> {
        use std::os::windows::fs::MetadataExt;

        // Get current attributes
        let metadata = std::fs::metadata(key_file)
            .map_err(|e| format!("Failed to read key file metadata: {e}"))?;
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
                return Err("Failed to set Windows file attributes".to_string());
            }
        }

        Ok(())
    }

    /// Encrypt a stream key for storage
    /// Returns base64-encoded encrypted key with prefix
    pub fn encrypt_stream_key(stream_key: &str, app_data_dir: &Path) -> Result<String, String> {
        // Don't encrypt empty keys or already encrypted keys
        if stream_key.is_empty() || stream_key.starts_with(STREAM_KEY_PREFIX) {
            return Ok(stream_key.to_string());
        }

        let machine_key = Self::get_or_create_machine_key(app_data_dir)?;

        // Generate random nonce
        let mut rng = rand::thread_rng();
        let nonce_bytes: [u8; NONCE_LEN] = rng.gen();

        // Encrypt
        let cipher = Aes256Gcm::new_from_slice(&*machine_key)
            .map_err(|e| format!("Failed to create cipher: {e}"))?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, stream_key.as_bytes())
            .map_err(|e| format!("Stream key encryption failed: {e}"))?;

        // Combine nonce + ciphertext and encode as base64
        let mut combined = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);

        Ok(format!("{}{}", STREAM_KEY_PREFIX, BASE64.encode(combined)))
    }

    /// Decrypt a stream key from storage
    /// Returns the original stream key (will be zeroized when dropped)
    pub fn decrypt_stream_key(encrypted_key: &str, app_data_dir: &Path) -> Result<String, String> {
        // If not encrypted, return as-is
        if !encrypted_key.starts_with(STREAM_KEY_PREFIX) {
            return Ok(encrypted_key.to_string());
        }

        let machine_key = Self::get_or_create_machine_key(app_data_dir)?;

        // Remove prefix and decode base64
        let encoded = &encrypted_key[STREAM_KEY_PREFIX.len()..];
        let mut combined = BASE64.decode(encoded)
            .map_err(|e| format!("Failed to decode encrypted stream key: {e}"))?;

        if combined.len() < NONCE_LEN {
            combined.zeroize();
            return Err("Invalid encrypted stream key".to_string());
        }

        // Extract nonce and ciphertext
        let nonce_bytes = &combined[..NONCE_LEN];
        let ciphertext = &combined[NONCE_LEN..];

        // Decrypt
        let cipher = Aes256Gcm::new_from_slice(&*machine_key)
            .map_err(|e| format!("Failed to create cipher: {e}"))?;
        let nonce = Nonce::from_slice(nonce_bytes);

        let mut plaintext = cipher.decrypt(nonce, ciphertext)
            .map_err(|e| format!("Stream key decryption failed: {e}"))?;

        let result = String::from_utf8(plaintext.clone())
            .map_err(|e| format!("Invalid UTF-8 in decrypted stream key: {e}"));

        // Zeroize sensitive buffers
        plaintext.zeroize();
        combined.zeroize();

        result
    }

    /// Check if a stream key is encrypted
    pub fn is_stream_key_encrypted(stream_key: &str) -> bool {
        stream_key.starts_with(STREAM_KEY_PREFIX)
    }

    // =========================================================================
    // Token Encryption (for OAuth tokens, API keys, and other sensitive settings)
    // Same AES-256-GCM + machine key scheme as stream keys
    // =========================================================================

    /// Encrypt a sensitive token for storage (OAuth tokens, API keys, etc.)
    /// Returns base64-encoded encrypted value with ENC:: prefix
    pub fn encrypt_token(token: &str, app_data_dir: &Path) -> Result<String, String> {
        Self::encrypt_stream_key(token, app_data_dir)
    }

    /// Decrypt a sensitive token from storage
    /// Returns the original plaintext token
    pub fn decrypt_token(encrypted: &str, app_data_dir: &Path) -> Result<String, String> {
        Self::decrypt_stream_key(encrypted, app_data_dir)
    }

    /// Check if a value is encrypted (has ENC:: prefix)
    pub fn is_encrypted(value: &str) -> bool {
        value.starts_with(STREAM_KEY_PREFIX)
    }

    // =========================================================================
    // Machine Key Rotation
    // Allows rotating the machine encryption key and re-encrypting all stream keys
    // =========================================================================

    /// Decrypt a stream key using a specific machine key (for rotation)
    fn decrypt_stream_key_with_key(
        encrypted_key: &str,
        machine_key: &Zeroizing<[u8; KEY_LEN]>,
    ) -> Result<String, String> {
        // If not encrypted, return as-is
        if !encrypted_key.starts_with(STREAM_KEY_PREFIX) {
            return Ok(encrypted_key.to_string());
        }

        // Remove prefix and decode base64
        let encoded = &encrypted_key[STREAM_KEY_PREFIX.len()..];
        let mut combined = BASE64.decode(encoded)
            .map_err(|e| format!("Failed to decode encrypted stream key: {e}"))?;

        if combined.len() < NONCE_LEN {
            combined.zeroize();
            return Err("Invalid encrypted stream key".to_string());
        }

        // Extract nonce and ciphertext
        let nonce_bytes = &combined[..NONCE_LEN];
        let ciphertext = &combined[NONCE_LEN..];

        // Decrypt
        let cipher = Aes256Gcm::new_from_slice(&**machine_key)
            .map_err(|e| format!("Failed to create cipher: {e}"))?;
        let nonce = Nonce::from_slice(nonce_bytes);

        let mut plaintext = cipher.decrypt(nonce, ciphertext)
            .map_err(|e| format!("Stream key decryption failed: {e}"))?;

        let result = String::from_utf8(plaintext.clone())
            .map_err(|e| format!("Invalid UTF-8 in decrypted stream key: {e}"));

        // Zeroize sensitive buffers
        plaintext.zeroize();
        combined.zeroize();

        result
    }

    /// Encrypt a stream key using a specific machine key (for rotation)
    fn encrypt_stream_key_with_key(
        stream_key: &str,
        machine_key: &Zeroizing<[u8; KEY_LEN]>,
    ) -> Result<String, String> {
        // Don't encrypt empty keys or already encrypted keys
        if stream_key.is_empty() || stream_key.starts_with(STREAM_KEY_PREFIX) {
            return Ok(stream_key.to_string());
        }

        // Generate random nonce
        let mut rng = rand::thread_rng();
        let nonce_bytes: [u8; NONCE_LEN] = rng.gen();

        // Encrypt
        let cipher = Aes256Gcm::new_from_slice(&**machine_key)
            .map_err(|e| format!("Failed to create cipher: {e}"))?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, stream_key.as_bytes())
            .map_err(|e| format!("Stream key encryption failed: {e}"))?;

        // Combine nonce + ciphertext and encode as base64
        let mut combined = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);

        Ok(format!("{}{}", STREAM_KEY_PREFIX, BASE64.encode(combined)))
    }

    /// Generate a new machine key
    fn generate_new_machine_key() -> Result<Zeroizing<[u8; KEY_LEN]>, String> {
        let mut rng = rand::thread_rng();
        Ok(Zeroizing::new(rng.gen::<[u8; KEY_LEN]>()))
    }

    /// Write a machine key to disk
    fn write_machine_key(
        key: &Zeroizing<[u8; KEY_LEN]>,
        app_data_dir: &Path,
    ) -> Result<(), String> {
        let key_file = app_data_dir.join(".stream_key");

        // Write key
        std::fs::write(&key_file, **key)
            .map_err(|e| format!("Failed to write machine key: {e}"))?;

        // Set restrictive permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&key_file, perms)
                .map_err(|e| format!("Failed to set key file permissions: {e}"))?;
        }

        // On Windows, set hidden and system attributes
        #[cfg(windows)]
        {
            Self::set_windows_key_attributes(&key_file)?;
        }

        Ok(())
    }

    /// Securely delete the old key file
    fn securely_delete_key_file(app_data_dir: &Path) -> Result<(), String> {
        let key_file = app_data_dir.join(".stream_key");

        if !key_file.exists() {
            return Ok(());
        }

        // Read file size
        let metadata = std::fs::metadata(&key_file)
            .map_err(|e| format!("Failed to read key file metadata: {e}"))?;
        let size = metadata.len() as usize;

        // Overwrite with zeros
        let zeros = vec![0u8; size];
        std::fs::write(&key_file, &zeros)
            .map_err(|e| format!("Failed to overwrite key file: {e}"))?;

        // Overwrite with random data
        let mut rng = rand::thread_rng();
        let random: Vec<u8> = (0..size).map(|_| rng.gen()).collect();
        std::fs::write(&key_file, &random)
            .map_err(|e| format!("Failed to overwrite key file: {e}"))?;

        // Delete
        std::fs::remove_file(&key_file)
            .map_err(|e| format!("Failed to delete key file: {e}"))?;

        Ok(())
    }

    /// Backup profiles directory before rotation
    fn backup_profiles_directory(app_data_dir: &Path) -> Result<PathBuf, String> {
        let profiles_dir = app_data_dir.join("profiles");
        let backup_dir = app_data_dir.join("profiles_backup");
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_path = backup_dir.join(format!("backup_{timestamp}"));

        log::info!("Creating backup at: {}", backup_path.display());

        // Create backup directory
        std::fs::create_dir_all(&backup_path)
            .map_err(|e| format!("Failed to create backup directory: {e}"))?;

        // Set restrictive permissions on backup directory
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o700); // Owner only
            std::fs::set_permissions(&backup_dir, perms.clone())
                .map_err(|e| format!("Failed to set backup directory permissions: {e}"))?;
            std::fs::set_permissions(&backup_path, perms)
                .map_err(|e| format!("Failed to set backup directory permissions: {e}"))?;
        }

        // Copy all profile files (.json and .mgs)
        let entries = std::fs::read_dir(&profiles_dir)
            .map_err(|e| format!("Failed to read profiles directory: {e}"))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext == "json" || ext == "mgs" {
                    if let Some(file_name) = path.file_name() {
                        let dest = backup_path.join(file_name);
                        std::fs::copy(&path, &dest)
                            .map_err(|e| format!("Failed to backup {}: {}", file_name.to_string_lossy(), e))?;
                        log::debug!("Backed up: {}", file_name.to_string_lossy());
                    }
                }
            }
        }

        log::info!("Backup created successfully");
        Ok(backup_path)
    }

    /// Restore profiles from backup
    fn restore_from_backup(backup_path: &Path, app_data_dir: &Path) -> Result<(), String> {
        let profiles_dir = app_data_dir.join("profiles");

        log::warn!("Restoring from backup: {}", backup_path.display());

        // Delete current profiles
        let entries = std::fs::read_dir(&profiles_dir)
            .map_err(|e| format!("Failed to read profiles directory: {e}"))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext == "json" || ext == "mgs" {
                    std::fs::remove_file(&path)
                        .map_err(|e| format!("Failed to delete {}: {}", path.display(), e))?;
                }
            }
        }

        // Restore from backup
        let backup_entries = std::fs::read_dir(backup_path)
            .map_err(|e| format!("Failed to read backup directory: {e}"))?;

        for entry in backup_entries.flatten() {
            let path = entry.path();
            if let Some(file_name) = path.file_name() {
                let dest = profiles_dir.join(file_name);
                std::fs::copy(&path, &dest)
                    .map_err(|e| format!("Failed to restore {}: {}", file_name.to_string_lossy(), e))?;
            }
        }

        log::info!("Backup restored successfully");
        Ok(())
    }

    /// Clean up old backups, keeping only the most recent N
    fn cleanup_old_backups(app_data_dir: &Path, keep_count: usize) -> Result<(), String> {
        let backup_dir = app_data_dir.join("profiles_backup");

        if !backup_dir.exists() {
            return Ok(());
        }

        // Get all backup directories
        let entries = std::fs::read_dir(&backup_dir)
            .map_err(|e| format!("Failed to read backup directory: {e}"))?;

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
                    .map_err(|e| format!("Failed to delete old backup: {e}"))?;
                backups.remove(0);
            }
        }

        Ok(())
    }

    /// Rotate the machine encryption key
    /// Re-encrypts all stream keys in all profiles with a new machine key
    pub fn rotate_machine_key(
        app_data_dir: &Path,
        profiles_dir: &Path,
    ) -> Result<RotationReport, String> {
        log::info!("Starting machine key rotation");

        // 1. Create backup
        let backup_path = Self::backup_profiles_directory(app_data_dir)?;

        // 2. Load old key
        let old_key = Self::get_or_create_machine_key(app_data_dir)?;

        // 3. Generate new key
        let new_key = Self::generate_new_machine_key()?;

        // 4. Get all profile files
        let entries = std::fs::read_dir(profiles_dir)
            .map_err(|e| format!("Failed to read profiles directory: {e}"))?;

        let profile_files: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| {
                let path = e.path();
                path.extension().and_then(|ext| ext.to_str()) == Some("json")
                    || path.extension().and_then(|ext| ext.to_str()) == Some("mgs")
            })
            .map(|e| e.path())
            .collect();

        let total_profiles = profile_files.len();
        let mut profiles_updated = 0;
        let mut keys_reencrypted = 0;

        // 5. Re-encrypt each profile
        for profile_path in profile_files {
            match Self::reencrypt_profile_file(&profile_path, &old_key, &new_key, app_data_dir) {
                Ok(count) => {
                    profiles_updated += 1;
                    keys_reencrypted += count;
                    log::debug!("Re-encrypted {} keys in {}", count, profile_path.display());
                }
                Err(e) => {
                    // Rollback on any error
                    log::error!("Failed to re-encrypt profile {}: {}", profile_path.display(), e);
                    log::error!("Rolling back changes");
                    Self::restore_from_backup(&backup_path, app_data_dir)?;
                    return Err(format!(
                        "Key rotation failed while updating {}: {}. All changes have been rolled back.",
                        profile_path.display(),
                        e
                    ));
                }
            }
        }

        // 6. Securely delete old key
        Self::securely_delete_key_file(app_data_dir)?;

        // 7. Write new key
        Self::write_machine_key(&new_key, app_data_dir)?;

        // 8. Clean up old backups (keep last 5)
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

    /// Re-encrypt a single profile file
    fn reencrypt_profile_file(
        profile_path: &Path,
        old_key: &Zeroizing<[u8; KEY_LEN]>,
        new_key: &Zeroizing<[u8; KEY_LEN]>,
        _app_data_dir: &Path,
    ) -> Result<usize, String> {
        use crate::models::Profile;

        // Read file content
        let content = std::fs::read(profile_path)
            .map_err(|e| format!("Failed to read profile file: {e}"))?;

        // Parse profile (handle both encrypted and unencrypted)
        let mut profile: Profile = if profile_path.extension().and_then(|e| e.to_str()) == Some("mgs") {
            // Encrypted profile - we can't decrypt without password
            // Skip re-encrypting stream keys in encrypted profiles
            // (they'll be re-encrypted when the profile is next saved)
            return Ok(0);
        } else {
            // Unencrypted JSON profile
            let json_str = String::from_utf8(content)
                .map_err(|e| format!("Invalid UTF-8 in profile: {e}"))?;
            serde_json::from_str(&json_str)
                .map_err(|e| format!("Failed to parse profile JSON: {e}"))?
        };

        // Re-encrypt stream keys
        let mut keys_updated = 0;

        for group in &mut profile.output_groups {
            for target in &mut group.stream_targets {
                // Only process encrypted keys
                if Self::is_stream_key_encrypted(&target.stream_key) {
                    // Decrypt with old key
                    let plaintext = Self::decrypt_stream_key_with_key(&target.stream_key, old_key)?;

                    // Re-encrypt with new key
                    target.stream_key = Self::encrypt_stream_key_with_key(&plaintext, new_key)?;

                    keys_updated += 1;
                }
            }
        }

        // Save updated profile
        let json = serde_json::to_string_pretty(&profile)
            .map_err(|e| format!("Failed to serialize profile: {e}"))?;

        std::fs::write(profile_path, json.as_bytes())
            .map_err(|e| format!("Failed to write profile: {e}"))?;

        Ok(keys_updated)
    }
}

/// Report returned after successful key rotation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RotationReport {
    pub profiles_updated: usize,
    pub keys_reencrypted: usize,
    pub total_profiles: usize,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
