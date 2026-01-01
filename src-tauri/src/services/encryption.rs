// Encryption Service
// Handles profile encryption using AES-256-GCM

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::Argon2;
use rand::Rng;

const SALT_LEN: usize = 32;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

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
            .map_err(|e| format!("Failed to create cipher: {}", e))?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, data)
            .map_err(|e| format!("Encryption failed: {}", e))?;

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
            .map_err(|e| format!("Failed to create cipher: {}", e))?;
        let nonce = Nonce::from_slice(nonce_bytes);

        cipher.decrypt(nonce, ciphertext)
            .map_err(|e| format!("Decryption failed: {}", e))
    }

    /// Derive a key from password using Argon2id
    fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; KEY_LEN], String> {
        let mut key = [0u8; KEY_LEN];

        Argon2::default()
            .hash_password_into(password.as_bytes(), salt, &mut key)
            .map_err(|e| format!("Key derivation failed: {}", e))?;

        Ok(key)
    }
}
