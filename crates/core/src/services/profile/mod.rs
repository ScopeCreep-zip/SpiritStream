//! Profile persistence service.
//!
//! Owns CRUD, encryption boundary, order-index file, validation, and the
//! `ProfileActivated` event payload. Split across focused sub-modules so
//! each concern lives in a file under 300 LOC:
//!
//! - `events.rs`        — `ProfileActivatedEvent` + `ActivatedObs` (DTOs)
//! - `validation.rs`    — name + settings-bounds validation, public constants
//! - `order_index.rs`   — order-index file I/O (drag-reorder support)
//! - `io.rs`            — file I/O: list, load, delete, is-encrypted, port-conflict
//! - `secret_fields.rs` — the walker enumerating every machine-key-encrypted field
//! - `security.rs`      — encryption boundary (per-field + whole-file envelope)
//!
//! The public symbols re-export through `services/profile_manager.rs` so the
//! historical import path `crate::services::profile_manager::ProfileManager`
//! keeps working unchanged.

mod events;
pub(super) mod io;
mod order_index;
pub(crate) mod secret_fields;
mod security;
mod validation;

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

pub use events::{ActivatedObs, ProfileActivatedEvent};
pub use validation::{BACKEND_PORT_MIN, DISCORD_COOLDOWN_SECONDS_MAX};

use super::AuditLogService;

/// Manages profile storage and retrieval.
pub struct ProfileManager {
    pub(super) profiles_dir: PathBuf,
    pub(super) app_data_dir: PathBuf,
    pub(super) order_index_dir: PathBuf,
    /// Audit-log handle, wired post-construction by `ServiceRegistry`
    /// (same pattern as `ChatManager::set_audit_log` / `ThemeManager`).
    /// G2: `ProfileSaved` + `ProfileDeleted` enum variants existed but
    /// were never emitted because the manager had no handle to record
    /// them. Wiring it here keeps emission single-source (every save/
    /// delete path goes through these methods).
    pub(super) audit_log: Arc<RwLock<Option<Arc<AuditLogService>>>>,
}

impl ProfileManager {
    /// Create a new ProfileManager with the given app data directory.
    pub fn new(app_data_dir: PathBuf) -> Self {
        let profiles_dir = app_data_dir.join("profiles");
        let order_index_dir = app_data_dir.join("indexes");
        std::fs::create_dir_all(&profiles_dir).ok();
        std::fs::create_dir_all(&order_index_dir).ok();
        Self {
            profiles_dir,
            app_data_dir,
            order_index_dir,
            audit_log: Arc::new(RwLock::new(None)),
        }
    }

    /// Wire the audit log after construction. Until this is called,
    /// `ProfileSaved` and `ProfileDeleted` are best-effort no-ops —
    /// matches the `ThemeManager` / `ChatManager` degraded-mode shape.
    pub fn set_audit_log(&self, audit: Arc<AuditLogService>) {
        match self.audit_log.write() {
            Ok(mut guard) => *guard = Some(audit),
            Err(e) => log::error!(
                "profile_manager audit_log write lock poisoned during set_audit_log: {e}"
            ),
        }
    }

    /// Read the wired audit-log handle. Returns `None` before
    /// `set_audit_log` runs (legitimate construction-order state) but
    /// logs and returns `None` on poisoned lock so audit gaps caused
    /// by an upstream panic surface in operator logs.
    pub(super) fn audit(&self) -> Option<Arc<AuditLogService>> {
        match self.audit_log.read() {
            Ok(g) => g.clone(),
            Err(e) => {
                log::error!(
                    "profile_manager audit_log read lock poisoned — audit entry dropped: {e}"
                );
                None
            }
        }
    }
}

#[cfg(test)]
mod permission_tests {
    use super::ProfileManager;
    use crate::models::{Profile, ProfileSettings, RtmpInput};

    fn fresh_dir() -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "spiritstream-profile-perm-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(p.join("profiles")).unwrap();
        p
    }

    /// Saved profile files (`.json` plaintext and `.mgs` encrypted) must
    /// land at mode 0600 on Unix. Profile files contain stream keys, OAuth
    /// tokens, OBS passwords, and webhook URLs — even the encrypted variant
    /// is sensitive metadata.
    #[cfg(unix)]
    #[tokio::test]
    async fn save_writes_profile_at_0600() {
        use std::os::unix::fs::PermissionsExt;
        let data_dir = fresh_dir();
        let mgr = ProfileManager::new(data_dir.clone());
        let profile = Profile {
            id: "phase69-test".into(),
            name: "phase69-test".into(),
            encrypted: false,
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
        mgr.save_with_key_encryption(&profile, None)
            .await
            .expect("save plaintext profile");
        let json_path = mgr.profiles_dir.join("phase69-test.json");
        let perms = std::fs::metadata(&json_path).unwrap().permissions();
        assert_eq!(
            perms.mode() & 0o777,
            0o600,
            "plaintext profile .json must be owner-only",
        );
        // Password must be ≥ PROFILE_PASSWORD_MIN_LENGTH (12 chars) — that's
        // the threat-model floor for the profile-encryption key.
        mgr.save_with_key_encryption(&profile, Some("test-pw-twelve"))
            .await
            .expect("save encrypted profile");
        let mgs_path = mgr.profiles_dir.join("phase69-test.mgs");
        let perms = std::fs::metadata(&mgs_path).unwrap().permissions();
        assert_eq!(
            perms.mode() & 0o777,
            0o600,
            "encrypted profile .mgs must be owner-only",
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
