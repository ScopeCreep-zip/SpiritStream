//! File-backed `SecretStore`.
//!
//! Stores each `(namespace, key)` entry as a single encrypted file
//! under `<app_data_dir>/secrets/`. Encryption is AES-256-GCM-SIV with
//! the per-machine key (`Encryption::encrypt_bytes_with_machine_key`).
//! File permissions are forced to `0600` on Unix at write time (Phase
//! 6.9 will add the regression test that asserts this on every write
//! path; this impl already does it for the bytes it owns).
//!
//! Entries are addressed by `SHA-256("<namespace>\0<key>")` rendered as
//! hex. The collision-resistant hash means the namespace/key combination
//! never enters the filesystem name verbatim, which sidesteps both path
//! traversal (`../`) and platform-specific filename rules.

use std::path::PathBuf;

use async_trait::async_trait;
use sha2::{Digest, Sha256};

use crate::errors::CoreError;
use crate::services::{write_owner_only_atomic, Encryption};
use crate::traits::SecretStore;

const SECRETS_DIRNAME: &str = "secrets";

pub struct EncryptedFileSecretStore {
    app_data_dir: PathBuf,
}

impl EncryptedFileSecretStore {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self { app_data_dir }
    }

    fn secrets_dir(&self) -> PathBuf {
        self.app_data_dir.join(SECRETS_DIRNAME)
    }

    fn ensure_secrets_dir(&self) -> Result<PathBuf, CoreError> {
        let dir = self.secrets_dir();
        std::fs::create_dir_all(&dir).map_err(|e| CoreError::Internal {
            context: format!("create secrets dir: {e}"),
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o700);
            let _ = std::fs::set_permissions(&dir, perms);
        }
        Ok(dir)
    }

    fn entry_path(&self, namespace: &str, key: &str) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(namespace.as_bytes());
        hasher.update(b"\0");
        hasher.update(key.as_bytes());
        let digest = hasher.finalize();
        self.secrets_dir().join(hex::encode(digest))
    }
}

/// AAD binding the ciphertext to its logical identity, so two secret
/// files swapped on disk cannot decrypt under each other's identity.
fn entry_aad(namespace: &str, key: &str) -> Vec<u8> {
    let mut aad = Vec::with_capacity(namespace.len() + 1 + key.len());
    aad.extend_from_slice(namespace.as_bytes());
    aad.push(0);
    aad.extend_from_slice(key.as_bytes());
    aad
}

#[async_trait]
impl SecretStore for EncryptedFileSecretStore {
    async fn get(&self, namespace: &str, key: &str) -> Result<Option<Vec<u8>>, CoreError> {
        let path = self.entry_path(namespace, key);
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path).map_err(|e| CoreError::Internal {
            context: format!("read secret: {e}"),
        })?;
        let aad = entry_aad(namespace, key);
        match Encryption::decrypt_bytes_with_machine_key_aad(&bytes, &aad, &self.app_data_dir) {
            Ok(plaintext) => Ok(Some(plaintext)),
            Err(aad_err) => {
                // One-time read migration: blobs written before AAD
                // binding have no associated data. If the legacy decrypt
                // succeeds, rewrite the entry AAD-bound so the legacy
                // path retires per entry (same V1→V2 auto-upgrade shape
                // the stream-key envelope uses).
                match Encryption::decrypt_bytes_with_machine_key(&bytes, &self.app_data_dir) {
                    Ok(plaintext) => {
                        log::info!(
                            "secret store: upgrading pre-AAD entry for namespace {namespace:?}"
                        );
                        let rebound = Encryption::encrypt_bytes_with_machine_key_aad(
                            &plaintext,
                            &aad,
                            &self.app_data_dir,
                        )?;
                        write_owner_only_atomic(&path, &rebound)?;
                        Ok(Some(plaintext))
                    }
                    Err(_) => Err(aad_err),
                }
            }
        }
    }

    async fn put(&self, namespace: &str, key: &str, value: &[u8]) -> Result<(), CoreError> {
        let _ = self.ensure_secrets_dir()?;
        let path = self.entry_path(namespace, key);
        let ciphertext = Encryption::encrypt_bytes_with_machine_key_aad(
            value,
            &entry_aad(namespace, key),
            &self.app_data_dir,
        )?;
        write_owner_only_atomic(&path, &ciphertext)
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<(), CoreError> {
        let path = self.entry_path(namespace, key);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(CoreError::Internal {
                context: format!("delete secret: {e}"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn store() -> (TempDir, EncryptedFileSecretStore) {
        let dir = TempDir::new().unwrap();
        let store = EncryptedFileSecretStore::new(dir.path().to_path_buf());
        (dir, store)
    }

    #[tokio::test]
    async fn put_then_get_roundtrips_bytes() {
        let (_dir, store) = store();
        store
            .put("oauth", "twitch.access_token", b"secret-bytes")
            .await
            .unwrap();
        let got = store.get("oauth", "twitch.access_token").await.unwrap();
        assert_eq!(got.as_deref(), Some(&b"secret-bytes"[..]));
    }

    #[tokio::test]
    async fn get_missing_returns_none() {
        let (_dir, store) = store();
        let got = store.get("oauth", "nonexistent").await.unwrap();
        assert_eq!(got, None);
    }

    #[tokio::test]
    async fn delete_is_idempotent() {
        let (_dir, store) = store();
        // Delete before put is a no-op (returns Ok).
        store.delete("oauth", "never-set").await.unwrap();
        store.put("oauth", "x", b"v").await.unwrap();
        store.delete("oauth", "x").await.unwrap();
        assert_eq!(store.get("oauth", "x").await.unwrap(), None);
        // Second delete also fine.
        store.delete("oauth", "x").await.unwrap();
    }

    #[tokio::test]
    async fn put_overwrites_existing_entry() {
        let (_dir, store) = store();
        store.put("k", "v", b"first").await.unwrap();
        store.put("k", "v", b"second").await.unwrap();
        let got = store.get("k", "v").await.unwrap();
        assert_eq!(got.as_deref(), Some(&b"second"[..]));
    }

    #[tokio::test]
    async fn namespace_isolates_entries() {
        let (_dir, store) = store();
        store.put("a", "key", b"alpha").await.unwrap();
        store.put("b", "key", b"beta").await.unwrap();
        assert_eq!(
            store.get("a", "key").await.unwrap().as_deref(),
            Some(&b"alpha"[..])
        );
        assert_eq!(
            store.get("b", "key").await.unwrap().as_deref(),
            Some(&b"beta"[..])
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn entries_are_written_0600() {
        use std::os::unix::fs::PermissionsExt;
        let (_dir, store) = store();
        store.put("ns", "k", b"v").await.unwrap();
        let path = store.entry_path("ns", "k");
        let perms = std::fs::metadata(&path).unwrap().permissions();
        assert_eq!(
            perms.mode() & 0o777,
            0o600,
            "secret file must be owner-read/write only",
        );
    }

    /// AAD regression: swapping two secret files on disk must NOT let
    /// each decrypt under the other's logical identity.
    #[tokio::test]
    async fn swapped_files_fail_to_decrypt() {
        let (_dir, store) = store();
        store.put("oauth", "twitch", b"twitch-token").await.unwrap();
        store.put("oauth", "kick", b"kick-token").await.unwrap();
        let p_twitch = store.entry_path("oauth", "twitch");
        let p_kick = store.entry_path("oauth", "kick");
        let twitch_bytes = std::fs::read(&p_twitch).unwrap();
        let kick_bytes = std::fs::read(&p_kick).unwrap();
        std::fs::write(&p_twitch, kick_bytes).unwrap();
        std::fs::write(&p_kick, twitch_bytes).unwrap();
        assert!(store.get("oauth", "twitch").await.is_err());
        assert!(store.get("oauth", "kick").await.is_err());
    }

    /// Pre-AAD blobs still read (one-time migration) and get rewritten
    /// AAD-bound on first access.
    #[tokio::test]
    async fn legacy_no_aad_entry_reads_and_upgrades() {
        let (dir, store) = store();
        store.ensure_secrets_dir().unwrap();
        let path = store.entry_path("ns", "legacy");
        let legacy_blob =
            Encryption::encrypt_bytes_with_machine_key(b"legacy-secret", dir.path()).unwrap();
        std::fs::write(&path, &legacy_blob).unwrap();

        let got = store.get("ns", "legacy").await.unwrap();
        assert_eq!(got.as_deref(), Some(&b"legacy-secret"[..]));
        // Rewritten on disk: bytes changed and now require the AAD.
        let upgraded = std::fs::read(&path).unwrap();
        assert_ne!(upgraded, legacy_blob);
        assert_eq!(
            store.get("ns", "legacy").await.unwrap().as_deref(),
            Some(&b"legacy-secret"[..])
        );
    }

    #[tokio::test]
    async fn ciphertext_is_not_plaintext_on_disk() {
        // Cross-check: the bytes on disk must NOT contain the plaintext.
        let (_dir, store) = store();
        let secret = b"plaintext-marker-DO-NOT-LEAK";
        store.put("ns", "k", secret).await.unwrap();
        let on_disk = std::fs::read(store.entry_path("ns", "k")).unwrap();
        assert!(
            !on_disk.windows(secret.len()).any(|w| w == secret),
            "plaintext leaked into ciphertext file",
        );
    }
}
