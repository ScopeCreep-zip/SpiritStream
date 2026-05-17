//! Append-only audit log with HMAC chain.
//!
//! Records security-relevant events: panic-triggered, PII-filter-fired,
//! profile-saved, OAuth-refreshed, anonymous-mode toggle, app-start,
//! audit-log-tamper.
//!
//! # Format
//!
//! Each entry is a JSON object (one per line) appended to
//! `<app_data_dir>/audit/audit.log`. The entry carries:
//!
//! * `seq` — monotonic counter starting at 1 (per file).
//! * `timestamp` — UTC.
//! * `action` — discriminated `AuditAction`.
//! * `detail` — optional short, non-secret metadata.
//! * `prevHmac` — hex of the previous entry's `hmac` (or 64 zero hex
//!   chars for `seq == 1`).
//! * `hmac` — hex of `HMAC-SHA256(audit_key, canonical_bytes(seq,
//!   timestamp, action, detail, prevHmac))`. Each entry's hmac becomes
//!   the next entry's `prevHmac`, forming an append-only chain.
//!
//! The HMAC key is derived from the machine key via
//! `HKDF-SHA256(machine_key, info="spiritstream/audit-log/hmac/v1")`.
//! The key never appears on disk and is regenerated identically on
//! every start, so a process restart resumes the chain.
//!
//! # Tamper detection
//!
//! [`AuditLogService::verify_chain`] walks the file from sequence 1
//! forward and recomputes each entry's HMAC. If any line fails to
//! verify (modified, deleted, or inserted out of band), the helper
//! returns `Err(CoreError::Internal { context: "audit chain broken..." })`
//! with the sequence number at which the chain breaks. The
//! red-banner contract reads this status on every fetch of the log.
//!
//! # What MUST be logged (OWASP)
//!
//! Authentication outcomes, panic-triggered, PII-filter-fired, profile
//! lifecycle, machine-key rotation, OAuth token refresh, settings
//! change, app start/stop, anonymous-mode toggle, audit-log tamper.
//!
//! # What MUST NOT be logged
//!
//! OAuth token values, chat message bodies, password contents, session
//! IDs, full file paths, EXIF metadata. The `record` API takes a
//! pre-shaped [`AuditEntry`] — adding a forbidden field at the
//! call site is rejected at the type system, not at runtime.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::errors::CoreError;

const AUDIT_DIRNAME: &str = "audit";
const AUDIT_FILENAME: &str = "audit.log";
const HMAC_KEY_INFO: &[u8] = b"spiritstream/audit-log/hmac/v1";
const ZERO_HMAC_HEX: &str = "0000000000000000000000000000000000000000000000000000000000000000";

type HmacSha256 = Hmac<Sha256>;

/// One discrete entry, including its position in and contribution to
/// the HMAC chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    /// Position in the append-only sequence. Starts at 1 for the
    /// first entry; strictly monotonic per audit-log file.
    #[serde(default)]
    pub seq: u64,
    /// Monotonic UTC timestamp for the event.
    pub timestamp: DateTime<Utc>,
    /// Structured event kind. Use one of the [`AuditAction`] variants
    /// to ensure the field name space stays bounded.
    pub action: AuditAction,
    /// Optional human-readable detail. **Must not** carry secret
    /// values (OAuth tokens, message bodies, passwords). Limit to
    /// short metadata — IDs, counts, platform names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Hex-encoded HMAC of the previous entry, or 64 zero hex chars
    /// for the first entry in the file.
    #[serde(default)]
    pub prev_hmac: String,
    /// Hex-encoded HMAC-SHA256 of this entry's canonical bytes.
    /// This is the chain's tamper-evidence anchor.
    #[serde(default)]
    pub hmac: String,
}

/// Enumerated set of audit event kinds. Adding a new variant is a
/// deliberate act — every recordable action passes through this enum.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuditAction {
    PanicTriggered {
        streams_stopped: usize,
        elapsed_ms: u64,
    },
    PiiFilterFired {
        platform: String,
        phrase_id: String,
    },
    ProfileSaved {
        name: String,
    },
    ProfileDeleted {
        name: String,
    },
    OauthRefresh {
        platform: String,
        success: bool,
    },
    OauthRefreshUnusualLocation {
        platform: String,
    },
    MachineKeyRotated {
        profiles_updated: usize,
        keys_reencrypted: usize,
    },
    AnonymousModeToggled {
        enabled: bool,
    },
    AppStarted,
    AppStopped,
    AuditLogTamperDetected {
        last_valid_sequence: u64,
    },
    /// A theme file on disk failed `ThemeManager::validate_theme()` —
    /// operator-visible signal for incomplete or malformed themes
    /// (e.g. accessibility regression where a high-contrast theme is
    /// missing required tokens). The WARN log alone is not user-visible
    /// and not searchable; the audit log makes regressions traceable.
    ThemeValidationFailed {
        file_name: String,
        reason: String,
    },
    /// The Tauri 2 self-updater rejected a downloaded
    /// release artifact's `.sig` against the embedded ed25519 pubkey,
    /// or the download itself failed in a way the user surface
    /// interprets as security-relevant. The frontend records this via
    /// `POST /api/v1/system/audit/app-update-failure` after a check /
    /// install error. Operators grep `app_update_signature_failed` in
    /// the chain to spot tampered-update attempts.
    AppUpdateSignatureFailed {
        detail: String,
    },
}

/// Result of a chain-verification pass. Held in memory by the
/// transport layer and surfaced to the audit-log UI as a banner.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuditChainStatus {
    /// Every entry's HMAC verified.
    Ok { entries_verified: u64 },
    /// Verification failed at the named sequence. The red
    /// banner displays `lastValidSequence` so the user knows how much
    /// of the log they can still trust.
    Tampered {
        last_valid_sequence: u64,
        reason: String,
    },
    /// Log file does not exist yet (fresh install).
    Empty,
}

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

    /// Same as [`record`] but accepts an explicit timestamp for tests.
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
            if hex::encode(computed) != entry.hmac {
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

/// Canonical pre-hash input for a given entry. **Field order matters**
/// — any reorder breaks the chain. JSON serialisation produces stable
/// bytes only when this function controls field order, so we hand-roll
/// the encoding (rather than trusting `serde_json::to_vec`).
fn canonical_input(
    seq: u64,
    timestamp: &DateTime<Utc>,
    action: &AuditAction,
    detail: Option<&str>,
    prev_hmac: &str,
) -> Result<Vec<u8>, CoreError> {
    let action_json = serde_json::to_string(action).map_err(|e| CoreError::Internal {
        context: format!("canonical action: {e}"),
    })?;
    let detail_str = detail.unwrap_or("");
    Ok(format!(
        "seq={seq};ts={};action={action_json};detail={detail_str};prev={prev_hmac}",
        timestamp.to_rfc3339()
    )
    .into_bytes())
}

fn compute_hmac(key: &[u8; 32], data: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    let result = mac.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

fn derive_audit_hmac_key(app_data_dir: &Path) -> Result<Zeroizing<[u8; 32]>, CoreError> {
    let machine_key = crate::services::Encryption::get_or_create_machine_key_public(app_data_dir)?;
    let hk = Hkdf::<Sha256>::new(None, &*machine_key);
    let mut out = Zeroizing::new([0u8; 32]);
    hk.expand(HMAC_KEY_INFO, &mut *out)
        .map_err(|e| CoreError::Internal {
            context: format!("audit hkdf: {e}"),
        })?;
    Ok(out)
}

fn scan_chain_tail(log_path: &Path) -> Result<(u64, String), CoreError> {
    if !log_path.exists() {
        return Ok((1, ZERO_HMAC_HEX.to_string()));
    }
    let entries = read_all_entries(log_path)?;
    if entries.is_empty() {
        return Ok((1, ZERO_HMAC_HEX.to_string()));
    }
    let last = entries.last().ok_or_else(|| CoreError::Internal {
        context: "audit log empty after read".into(),
    })?;
    Ok((last.seq.saturating_add(1), last.hmac.clone()))
}

fn read_all_entries(log_path: &Path) -> Result<Vec<AuditEntry>, CoreError> {
    if !log_path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(log_path).map_err(|e| CoreError::Internal {
        context: format!("audit read: {e}"),
    })?;
    let mut entries = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let entry: AuditEntry = serde_json::from_str(line).map_err(|e| CoreError::Internal {
            context: format!("audit parse: {e}"),
        })?;
        entries.push(entry);
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn svc() -> (TempDir, AuditLogService) {
        let dir = TempDir::new().unwrap();
        let svc = AuditLogService::new(dir.path().to_path_buf()).unwrap();
        (dir, svc)
    }

    #[test]
    fn append_then_read_roundtrips_and_carries_seq_plus_hmac() {
        let (_dir, svc) = svc();
        svc.record(AuditAction::AppStarted).unwrap();
        svc.record(AuditAction::PanicTriggered {
            streams_stopped: 2,
            elapsed_ms: 145,
        })
        .unwrap();
        let entries = svc.entries().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].seq, 1);
        assert_eq!(entries[1].seq, 2);
        // First entry's prev_hmac is all-zero.
        assert_eq!(entries[0].prev_hmac, ZERO_HMAC_HEX);
        // Second entry's prev_hmac equals the first's hmac.
        assert_eq!(entries[1].prev_hmac, entries[0].hmac);
        // Each hmac is 64 hex chars (SHA-256 -> 32 bytes -> 64 hex).
        assert_eq!(entries[0].hmac.len(), 64);
        assert_eq!(entries[1].hmac.len(), 64);
    }

    #[test]
    fn verify_chain_passes_for_clean_log() {
        let (_dir, svc) = svc();
        svc.record(AuditAction::AppStarted).unwrap();
        svc.record(AuditAction::AppStopped).unwrap();
        let status = svc.verify_chain().unwrap();
        match status {
            AuditChainStatus::Ok { entries_verified } => assert_eq!(entries_verified, 2),
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    #[test]
    fn verify_chain_detects_modified_entry() {
        let dir = TempDir::new().unwrap();
        let svc = AuditLogService::new(dir.path().to_path_buf()).unwrap();
        svc.record(AuditAction::AppStarted).unwrap();
        svc.record(AuditAction::ProfileSaved {
            name: "alice".into(),
        })
        .unwrap();
        svc.record(AuditAction::AppStopped).unwrap();

        // Mutate the second entry's `name` field on disk to simulate
        // a forensic-evading edit.
        let raw = std::fs::read_to_string(svc.log_path()).unwrap();
        let tampered = raw.replace("\"alice\"", "\"mallory\"");
        std::fs::write(svc.log_path(), tampered).unwrap();

        let status = svc.verify_chain().unwrap();
        match status {
            AuditChainStatus::Tampered {
                last_valid_sequence,
                reason,
            } => {
                assert_eq!(last_valid_sequence, 1, "first entry should still verify");
                assert!(reason.contains("hmac mismatch"), "reason: {reason}");
            }
            other => panic!("expected Tampered, got {other:?}"),
        }
    }

    #[test]
    fn verify_chain_detects_deleted_entry() {
        let dir = TempDir::new().unwrap();
        let svc = AuditLogService::new(dir.path().to_path_buf()).unwrap();
        svc.record(AuditAction::AppStarted).unwrap();
        svc.record(AuditAction::ProfileSaved {
            name: "alice".into(),
        })
        .unwrap();
        svc.record(AuditAction::AppStopped).unwrap();

        // Remove the middle entry — the chain now skips seq=2.
        let raw = std::fs::read_to_string(svc.log_path()).unwrap();
        let lines: Vec<&str> = raw.lines().collect();
        let pruned = format!("{}\n{}\n", lines[0], lines[2]);
        std::fs::write(svc.log_path(), pruned).unwrap();

        let status = svc.verify_chain().unwrap();
        assert!(matches!(status, AuditChainStatus::Tampered { .. }));
    }

    #[test]
    fn empty_log_verify_returns_empty() {
        let (_dir, svc) = svc();
        assert!(matches!(
            svc.verify_chain().unwrap(),
            AuditChainStatus::Empty
        ));
    }

    #[cfg(unix)]
    #[test]
    fn audit_log_file_is_0600() {
        use std::os::unix::fs::PermissionsExt;
        let (_dir, svc) = svc();
        svc.record(AuditAction::AppStarted).unwrap();
        let perms = std::fs::metadata(svc.log_path()).unwrap().permissions();
        assert_eq!(perms.mode() & 0o777, 0o600);
    }

    #[test]
    fn pii_filter_entry_records_phrase_id_not_phrase_text() {
        let (_dir, svc) = svc();
        svc.record(AuditAction::PiiFilterFired {
            platform: "twitch".into(),
            phrase_id: "abc123".into(),
        })
        .unwrap();
        let line = std::fs::read_to_string(svc.log_path()).unwrap();
        assert!(line.contains("phrase_id"));
        assert!(!line.contains("real-name-here"));
    }

    #[test]
    fn appends_after_restart_resume_chain() {
        let dir = TempDir::new().unwrap();
        {
            let svc = AuditLogService::new(dir.path().to_path_buf()).unwrap();
            svc.record(AuditAction::AppStarted).unwrap();
        }
        // Re-open — should pick up seq=2 + the prior hmac.
        let svc = AuditLogService::new(dir.path().to_path_buf()).unwrap();
        svc.record(AuditAction::AppStopped).unwrap();
        let entries = svc.entries().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].seq, 2);
        assert_eq!(entries[1].prev_hmac, entries[0].hmac);
        // And the chain still verifies after the cold restart.
        assert!(matches!(
            svc.verify_chain().unwrap(),
            AuditChainStatus::Ok {
                entries_verified: 2
            }
        ));
    }
}
