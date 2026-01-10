// Encryption Service
// Handles profile encryption using AES-256-GCM

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{Argon2, Algorithm, Version, Params};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::Rng;
use std::path::Path;
use zeroize::{Zeroize, Zeroizing};

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
            std::fs::write(&key_file, &*key)
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
}
