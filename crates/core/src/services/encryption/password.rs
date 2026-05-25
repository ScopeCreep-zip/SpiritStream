use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use aes_gcm_siv::Aes256GcmSiv;
use rand::Rng;

use crate::errors::CoreError;

use super::kdf::derive_key;
use super::{internal, NONCE_LEN, SALT_LEN};

impl super::Encryption {
    /// Encrypt data with a password. Always writes V2 (AES-256-GCM-SIV).
    /// Layout: `[salt:32][nonce:12][ciphertext]` — the 4-byte version magic
    /// is the caller's responsibility (profile_manager prepends `MGL2`).
    pub fn encrypt(data: &[u8], password: &str) -> Result<Vec<u8>, CoreError> {
        let mut rng = rand::thread_rng();
        let salt: [u8; SALT_LEN] = rng.gen();
        let nonce_bytes: [u8; NONCE_LEN] = rng.gen();

        let key = derive_key(password, &salt)?;

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

        let key = derive_key(password, salt)?;

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

        let key = derive_key(password, salt)?;

        let cipher = Aes256GcmSiv::new_from_slice(&*key)
            .map_err(|e| internal("Failed to create cipher", e))?;
        let nonce = aes_gcm_siv::Nonce::from_slice(nonce_bytes);

        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| CoreError::PasswordIncorrect)
    }
}
