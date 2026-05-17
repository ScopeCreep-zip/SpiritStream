//! Secret-storage selection.
//!
//! Two impls live side-by-side as independent modules:
//!
//! * [`KeyringSecretStore`] — wraps the OS keyring (macOS Keychain,
//!   Windows Credential Manager, Linux Secret Service).
//! * [`EncryptedFileSecretStore`] — AES-256-GCM-SIV under the
//!   per-machine key, for Docker / headless / any environment that
//!   can't host an OS keyring.
//!
//! **Exactly one is selected at startup** by [`build_secret_store`] and
//! held as `Arc<dyn SecretStore>` for the process lifetime. There is no
//! runtime fallback chain between them — the binary picks once and runs
//! that impl exclusively (per the "no fallback" rule in
//! `feedback_no_legacy.md`).
//!
//! Operators can force a choice with `SPIRITSTREAM_SECRET_STORE=keyring|file`.
//! With no override, a one-shot canary round-trip on the OS keyring
//! decides: success → keyring, any error → file.

use std::path::Path;
use std::sync::Arc;

use crate::traits::SecretStore;

mod encrypted_file_store;
mod keyring_store;

pub use encrypted_file_store::EncryptedFileSecretStore;
pub use keyring_store::KeyringSecretStore;

/// Which `SecretStore` impl the factory picked. Logged at startup; never
/// changes for the rest of the process lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretStoreKind {
    Keyring,
    File,
}

/// Build the single `SecretStore` impl this process will use.
///
/// * `app_data_dir` — where the file-backed impl writes its blobs if
///   chosen.
/// * `override_kind` — value of `SPIRITSTREAM_SECRET_STORE`. Recognised:
///   `"keyring"`, `"file"`. Anything else (including `None`) falls
///   through to the auto-detect probe.
pub fn build_secret_store(
    app_data_dir: &Path,
    override_kind: Option<&str>,
) -> Arc<dyn SecretStore> {
    let chosen = resolve_kind(override_kind);
    log::info!("Secret store: {chosen:?}");
    match chosen {
        SecretStoreKind::Keyring => Arc::new(KeyringSecretStore::new()),
        SecretStoreKind::File => {
            Arc::new(EncryptedFileSecretStore::new(app_data_dir.to_path_buf()))
        }
    }
}

/// Pure decision function — returns the kind without constructing it.
/// Exposed for tests so they can verify the override + probe logic
/// without touching the OS keyring or filesystem.
pub fn resolve_kind(override_kind: Option<&str>) -> SecretStoreKind {
    match override_kind.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("keyring") => SecretStoreKind::Keyring,
        Some("file") => SecretStoreKind::File,
        _ => probe_keyring(),
    }
}

/// One-shot probe: try a full keyring round-trip (write → read → delete)
/// of a sentinel canary entry. Any error indicates the keyring is not
/// available in this environment (no D-Bus, sandboxed container, etc.)
/// and the factory falls through to the file impl.
fn probe_keyring() -> SecretStoreKind {
    const SERVICE: &str = "spiritstream";
    const ACCOUNT: &str = "__probe__";
    const CANARY: &str = "probe";

    let attempt = || -> Result<bool, keyring::Error> {
        let entry = keyring::Entry::new(SERVICE, ACCOUNT)?;
        entry.set_password(CANARY)?;
        let read = entry.get_password()?;
        // Best-effort cleanup. If delete fails (e.g. permissions on a
        // platform that requires explicit confirmation), the probe still
        // counted as success because read worked.
        let _ = entry.delete_credential();
        Ok(read == CANARY)
    };

    match attempt() {
        Ok(true) => SecretStoreKind::Keyring,
        Ok(false) => {
            log::warn!(
                "Secret store: keyring round-trip read mismatched written value, using file impl"
            );
            SecretStoreKind::File
        }
        Err(e) => {
            log::info!("Secret store: keyring unavailable ({e}); using file impl");
            SecretStoreKind::File
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_override_keyring_wins() {
        // Test the override branch directly. The probe is not invoked.
        assert_eq!(resolve_kind(Some("keyring")), SecretStoreKind::Keyring);
        assert_eq!(resolve_kind(Some("KEYRING")), SecretStoreKind::Keyring);
    }

    #[test]
    fn explicit_override_file_wins() {
        assert_eq!(resolve_kind(Some("file")), SecretStoreKind::File);
        assert_eq!(resolve_kind(Some("FILE")), SecretStoreKind::File);
    }

    #[test]
    fn unrecognised_override_falls_through_to_probe() {
        // We can't assert which kind the probe returns in CI (depends on
        // host capabilities) — just that the function returns SOMETHING
        // and doesn't panic. The override path is what we control.
        let _ = resolve_kind(Some("nonsense"));
        let _ = resolve_kind(None);
    }
}
