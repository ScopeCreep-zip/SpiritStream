//! OS-keyring backed `SecretStore`.
//!
//! Uses `keyring = "3"` to reach macOS Keychain, Windows Credential
//! Manager, and Linux Secret Service. The keyring API stores strings,
//! so binary values are base64-encoded on the way in and decoded on the
//! way out.
//!
//! There is no fallback to another backend — if the keyring is
//! unavailable in this environment, the factory at startup already
//! chose the file impl instead.

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

use crate::errors::CoreError;
use crate::traits::SecretStore;

const SERVICE: &str = "spiritstream";

pub struct KeyringSecretStore;

impl KeyringSecretStore {
    pub fn new() -> Self {
        Self
    }

    fn account(namespace: &str, key: &str) -> String {
        // The keyring "account" field is presented to the user inside
        // their OS keyring UI, so keep it human-readable. We've already
        // validated namespace/key elsewhere — no separator escaping
        // needed beyond joining with "::".
        format!("{namespace}::{key}")
    }

    fn map_err(prefix: &str, e: keyring::Error) -> CoreError {
        CoreError::Internal {
            context: format!("{prefix}: {e}"),
        }
    }
}

impl Default for KeyringSecretStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SecretStore for KeyringSecretStore {
    async fn get(&self, namespace: &str, key: &str) -> Result<Option<Vec<u8>>, CoreError> {
        let account = Self::account(namespace, key);
        let entry = keyring::Entry::new(SERVICE, &account)
            .map_err(|e| Self::map_err("keyring entry", e))?;
        match entry.get_password() {
            Ok(encoded) => {
                let bytes = BASE64
                    .decode(encoded.as_bytes())
                    .map_err(|e| CoreError::Internal {
                        context: format!("keyring base64 decode: {e}"),
                    })?;
                Ok(Some(bytes))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(Self::map_err("keyring get", e)),
        }
    }

    async fn put(&self, namespace: &str, key: &str, value: &[u8]) -> Result<(), CoreError> {
        let account = Self::account(namespace, key);
        let entry = keyring::Entry::new(SERVICE, &account)
            .map_err(|e| Self::map_err("keyring entry", e))?;
        let encoded = BASE64.encode(value);
        entry
            .set_password(&encoded)
            .map_err(|e| Self::map_err("keyring put", e))
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<(), CoreError> {
        let account = Self::account(namespace, key);
        let entry = keyring::Entry::new(SERVICE, &account)
            .map_err(|e| Self::map_err("keyring entry", e))?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            // Idempotent delete — absent entry is success.
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(Self::map_err("keyring delete", e)),
        }
    }
}
