// Encryption Service
// Handles profile encryption using AES-256-GCM

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::Argon2;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::Rng;
use std::path::Path;

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
        let cipher = Aes256Gcm::new_from_slice(&key)
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
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| format!("Failed to create cipher: {e}"))?;
        let nonce = Nonce::from_slice(nonce_bytes);

        cipher.decrypt(nonce, ciphertext)
            .map_err(|e| format!("Decryption failed: {e}"))
    }

    /// Derive a key from password using Argon2id
    fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; KEY_LEN], String> {
        let mut key = [0u8; KEY_LEN];

        Argon2::default()
            .hash_password_into(password.as_bytes(), salt, &mut key)
            .map_err(|e| format!("Key derivation failed: {e}"))?;

        Ok(key)
    }

    // =========================================================================
    // Stream Key Encryption (for encrypt_stream_keys setting)
    // Uses a machine-specific key stored in the app data directory
    // =========================================================================

    /// Get or create the machine-specific encryption key for stream keys
    fn get_or_create_machine_key(app_data_dir: &Path) -> Result<[u8; KEY_LEN], String> {
        let key_file = app_data_dir.join(".stream_key");

        if key_file.exists() {
            // Read existing key
            let key_data = std::fs::read(&key_file)
                .map_err(|e| format!("Failed to read machine key: {e}"))?;

            if key_data.len() != KEY_LEN {
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

            let mut key = [0u8; KEY_LEN];
            key.copy_from_slice(&key_data);
            Ok(key)
        } else {
            // Generate new key
            let mut rng = rand::thread_rng();
            let key: [u8; KEY_LEN] = rng.gen();

            // Save it
            std::fs::write(&key_file, key)
                .map_err(|e| format!("Failed to save machine key: {e}"))?;

            // Set restrictive permissions on the new key file
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o600);
                std::fs::set_permissions(&key_file, perms)
                    .map_err(|e| format!("Failed to set key file permissions: {e}"))?;
            }

            Ok(key)
        }
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
        let cipher = Aes256Gcm::new_from_slice(&machine_key)
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
    /// Returns the original stream key
    pub fn decrypt_stream_key(encrypted_key: &str, app_data_dir: &Path) -> Result<String, String> {
        // If not encrypted, return as-is
        if !encrypted_key.starts_with(STREAM_KEY_PREFIX) {
            return Ok(encrypted_key.to_string());
        }

        let machine_key = Self::get_or_create_machine_key(app_data_dir)?;

        // Remove prefix and decode base64
        let encoded = &encrypted_key[STREAM_KEY_PREFIX.len()..];
        let combined = BASE64.decode(encoded)
            .map_err(|e| format!("Failed to decode encrypted stream key: {e}"))?;

        if combined.len() < NONCE_LEN {
            return Err("Invalid encrypted stream key".to_string());
        }

        // Extract nonce and ciphertext
        let nonce_bytes = &combined[..NONCE_LEN];
        let ciphertext = &combined[NONCE_LEN..];

        // Decrypt
        let cipher = Aes256Gcm::new_from_slice(&machine_key)
            .map_err(|e| format!("Failed to create cipher: {e}"))?;
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = cipher.decrypt(nonce, ciphertext)
            .map_err(|e| format!("Stream key decryption failed: {e}"))?;

        String::from_utf8(plaintext)
            .map_err(|e| format!("Invalid UTF-8 in decrypted stream key: {e}"))
    }

    /// Check if a stream key is encrypted
    pub fn is_stream_key_encrypted(stream_key: &str) -> bool {
        stream_key.starts_with(STREAM_KEY_PREFIX)
    }
}
