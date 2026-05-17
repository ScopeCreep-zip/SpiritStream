//! Pluggable secret storage contract.
//!
//! Two concrete impls ship. Exactly one is selected at
//! startup by `build_secret_store(...)` and held as `Arc<dyn SecretStore>`
//! for the process lifetime — there is no runtime fallback chain:
//! - `KeyringSecretStore` via `keyring-rs` (macOS Keychain, Windows
//!   Credential Manager, Linux Secret Service).
//! - `EncryptedFileSecretStore` wrapping the AES-256-GCM-SIV +
//!   Argon2id machine-key approach (Docker, headless Linux without
//!   D-Bus, other environments that cannot host an OS keyring).
//!
//! The trait lives in core so service code can be written against the
//! abstraction; the selection logic lives in the secret_store module.

use async_trait::async_trait;

use crate::CoreError;

#[async_trait]
pub trait SecretStore: Send + Sync {
    /// Returns the stored secret bytes, or `Ok(None)` if absent.
    async fn get(&self, namespace: &str, key: &str) -> Result<Option<Vec<u8>>, CoreError>;

    /// Stores `value`, overwriting any prior entry.
    async fn put(&self, namespace: &str, key: &str, value: &[u8]) -> Result<(), CoreError>;

    /// Removes the entry. No-op if absent.
    async fn delete(&self, namespace: &str, key: &str) -> Result<(), CoreError>;

    /// Zeroizes in-memory caches without touching persistent storage.
    /// Invoked by `SafetyService::panic`.
    async fn purge_caches(&self) {}
}
