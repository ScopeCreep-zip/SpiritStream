use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use zeroize::Zeroizing;

use crate::errors::CoreError;

use super::actions::{AuditAction, AuditChainStatus, AuditEntry};
use super::helpers::{
    canonical_input, compute_hmac, derive_audit_hmac_key, read_all_entries, scan_chain_tail,
};
use super::{AUDIT_DIRNAME, AUDIT_FILENAME, ZERO_HMAC_HEX};

/// Append-only audit log writer. Construct once via
/// [`AuditLogService::new`] and share as `Arc<...>` across transports.
///
/// The HMAC key is derived from the per-machine key at construction
/// time. The service caches the *next* sequence number and the *last*
/// hmac in memory so successive writes are constant-time.
pub struct AuditLogService {
    log_path: PathBuf,
    state: Mutex<ChainState>,
    hmac_key: Zeroizing<[u8; 32]>,
}

struct ChainState {
    next_seq: u64,
    last_hmac: String, // hex; "0000..." before the first entry
}

impl AuditLogService {
    /// Create (or open) the audit log. Derives the HMAC chain key
    /// from the per-machine key via HKDF-SHA256 and scans any
    /// existing entries to resume the chain at the right sequence.
    pub fn new(app_data_dir: PathBuf) -> Result<Self, CoreError> {
        let dir = app_data_dir.join(AUDIT_DIRNAME);
        std::fs::create_dir_all(&dir).map_err(|e| CoreError::Internal {
            context: format!("audit log dir: {e}"),
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
        }
        let log_path = dir.join(AUDIT_FILENAME);

        let hmac_key = derive_audit_hmac_key(&app_data_dir)?;
        let (next_seq, last_hmac) = scan_chain_tail(&log_path)?;

        Ok(Self {
            log_path,
            state: Mutex::new(ChainState {
                next_seq,
                last_hmac,
            }),
            hmac_key,
        })
    }

    /// Path to the audit log file. Exposed for the audit-log UI
    /// endpoint and the tamper-detector.
    pub fn log_path(&self) -> &Path {
        &self.log_path
    }

    /// Append a single entry. The chain HMAC is computed inside the
    /// `state` lock so concurrent writers don't race on the
    /// next-seq / prev-hmac pair.
    pub fn record(&self, action: AuditAction) -> Result<(), CoreError> {
        self.record_with_timestamp(action, Utc::now())
    }

    /// Same as [`Self::record`] but accepts an explicit timestamp for tests.
    pub fn record_with_timestamp(
        &self,
        action: AuditAction,
        timestamp: DateTime<Utc>,
    ) -> Result<(), CoreError> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let seq = state.next_seq;
        let prev_hmac = state.last_hmac.clone();

        let canonical = canonical_input(seq, &timestamp, &action, None, &prev_hmac)?;
        let hmac_bytes = compute_hmac(&self.hmac_key, &canonical);
        let hmac_hex = hex::encode(hmac_bytes);

        let entry = AuditEntry {
            seq,
            timestamp,
            action,
            detail: None,
            prev_hmac,
            hmac: hmac_hex.clone(),
        };
        let mut line = serde_json::to_string(&entry).map_err(|e| CoreError::Internal {
            context: format!("audit serialize: {e}"),
        })?;
        line.push('\n');

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .map_err(|e| CoreError::Internal {
                context: format!("audit open: {e}"),
            })?;
        use std::io::Write;
        file.write_all(line.as_bytes())
            .map_err(|e| CoreError::Internal {
                context: format!("audit write: {e}"),
            })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ =
                std::fs::set_permissions(&self.log_path, std::fs::Permissions::from_mode(0o600));
        }
        // I8: Windows ACL inheritance from the parent data directory
        // is usually enough on a standard install, but flag the file
        // as HIDDEN + SYSTEM so it doesn't surface in a casual
        // Explorer browse. Mirrors `secure_io::harden_perms`.
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if let Ok(meta) = std::fs::metadata(&self.log_path) {
                const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
                const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
                let attrs = meta.file_attributes() | FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM;
                use std::os::windows::ffi::OsStrExt;
                let wide: Vec<u16> = self
                    .log_path
                    .as_os_str()
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                unsafe {
                    let _ = winapi::um::fileapi::SetFileAttributesW(wide.as_ptr(), attrs);
                }
            }
        }

        state.next_seq = seq.saturating_add(1);
        state.last_hmac = hmac_hex;
        Ok(())
    }

    /// Read every entry currently on disk. Lines that fail to
    /// deserialise are returned as `Err`.
    pub fn entries(&self) -> Result<Vec<AuditEntry>, CoreError> {
        read_all_entries(&self.log_path)
    }

    /// Recompute the HMAC chain from sequence 1 forward and report
    /// whether every entry verifies. The red-banner UI calls this on
    /// every audit-log fetch.
    pub fn verify_chain(&self) -> Result<AuditChainStatus, CoreError> {
        if !self.log_path.exists() {
            return Ok(AuditChainStatus::Empty);
        }
        let entries = read_all_entries(&self.log_path)?;
        if entries.is_empty() {
            return Ok(AuditChainStatus::Empty);
        }

        let mut expected_seq: u64 = 1;
        let mut expected_prev: String = ZERO_HMAC_HEX.to_string();
        let mut last_valid: u64 = 0;
        for entry in &entries {
            if entry.seq != expected_seq {
                return Ok(AuditChainStatus::Tampered {
                    last_valid_sequence: last_valid,
                    reason: format!("expected seq {expected_seq}, got {}", entry.seq),
                });
            }
            if entry.prev_hmac != expected_prev {
                return Ok(AuditChainStatus::Tampered {
                    last_valid_sequence: last_valid,
                    reason: format!("prev_hmac mismatch at seq {}", entry.seq),
                });
            }
            let canonical = canonical_input(
                entry.seq,
                &entry.timestamp,
                &entry.action,
                entry.detail.as_deref(),
                &entry.prev_hmac,
            )?;
            let computed = compute_hmac(&self.hmac_key, &canonical);
            // Constant-time comparison so the verification path doesn't
            // leak how many leading bytes a forged hmac matched. Hex on
            // both sides keeps the comparison stable across encoding
            // differences and (since hex is fixed-width 64 chars for a
            // 32-byte HMAC-SHA256) a length mismatch already implies
            // tamper without needing the byte-level compare.
            use subtle::ConstantTimeEq;
            let computed_hex = hex::encode(computed);
            let tampered = computed_hex.len() != entry.hmac.len()
                || !bool::from(computed_hex.as_bytes().ct_eq(entry.hmac.as_bytes()));
            if tampered {
                return Ok(AuditChainStatus::Tampered {
                    last_valid_sequence: last_valid,
                    reason: format!("hmac mismatch at seq {}", entry.seq),
                });
            }
            last_valid = entry.seq;
            expected_seq = entry.seq.saturating_add(1);
            expected_prev = entry.hmac.clone();
        }
        Ok(AuditChainStatus::Ok {
            entries_verified: last_valid,
        })
    }
}
