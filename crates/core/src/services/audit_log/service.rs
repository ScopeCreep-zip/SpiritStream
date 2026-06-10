use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use zeroize::Zeroizing;

use crate::errors::CoreError;
use crate::traits::SecretStore;

use super::actions::{AuditAction, AuditChainStatus, AuditEntry};
use super::anchor::AnchorHandle;
use super::helpers::{
    canonical_input, compute_hmac, day_of, derive_audit_hmac_key, derive_day_key,
    read_entries_lenient,
};
use super::{AUDIT_DIRNAME, AUDIT_FILENAME, ZERO_HMAC_HEX};

/// Append-only audit log writer. Construct once via
/// [`AuditLogService::new`] and share as `Arc<...>` across transports.
///
/// Keys: a master key derives from the per-machine key at construction;
/// each entry is HMAC'd under a per-day key derived from the master
/// (key evolution per UTC day). The tail anchor lives in the
/// [`SecretStore`] so truncation/deletion of the log file is detected.
pub struct AuditLogService {
    log_path: PathBuf,
    state: Mutex<ChainState>,
    master_key: Zeroizing<[u8; 32]>,
    day_keys: Mutex<HashMap<String, Zeroizing<[u8; 32]>>>,
    anchor: AnchorHandle,
    /// Set when construction itself observed an anchor breach (log
    /// truncated below the anchor, or deleted while an anchor exists).
    /// Sticky for the process lifetime: recording continues (the
    /// entries ARE the safety record) but every status read reports
    /// Tampered until an operator investigates.
    startup_breach: Mutex<Option<String>>,
}

struct ChainState {
    next_seq: u64,
    last_hmac: String, // hex; "0000..." before the first entry
}

enum OpeningPlan {
    Resume {
        next_seq: u64,
        last_hmac: String,
        breach: Option<String>,
    },
    /// Chain verified under the legacy single-key scheme → archive it
    /// and start fresh, recording `ChainMigrated`.
    Migrate { archived_entries: u64 },
    /// Unparseable line found → quarantine the file and start fresh,
    /// recording `ChainQuarantined`. Startup must never brick on a
    /// corrupt log: this is the panic button's audit trail.
    Quarantine { reason: String },
}

impl AuditLogService {
    /// Create (or open) the audit log.
    pub fn new(app_data_dir: PathBuf, secrets: Arc<dyn SecretStore>) -> Result<Self, CoreError> {
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

        let master_key = derive_audit_hmac_key(&app_data_dir)?;
        let (anchor, loaded_anchor) = AnchorHandle::start(secrets);

        let plan = Self::opening_plan(&log_path, &master_key, loaded_anchor.as_ref())?;

        let (state, breach, opening_action) = match plan {
            OpeningPlan::Resume {
                next_seq,
                last_hmac,
                breach,
            } => (
                ChainState {
                    next_seq,
                    last_hmac,
                },
                breach,
                None,
            ),
            OpeningPlan::Migrate { archived_entries } => {
                let archive = dir.join(format!("{AUDIT_FILENAME}.v1-archive"));
                std::fs::rename(&log_path, &archive).map_err(|e| CoreError::Internal {
                    context: format!("audit archive rename: {e}"),
                })?;
                log::warn!(
                    "audit log migrated to per-day chain keys; legacy chain archived at {}",
                    archive.display()
                );
                anchor.clear();
                (
                    ChainState {
                        next_seq: 1,
                        last_hmac: ZERO_HMAC_HEX.to_string(),
                    },
                    None,
                    Some(AuditAction::ChainMigrated { archived_entries }),
                )
            }
            OpeningPlan::Quarantine { reason } => {
                let quarantine = dir.join(format!(
                    "{AUDIT_FILENAME}.quarantined-{}",
                    Utc::now().format("%Y%m%dT%H%M%SZ")
                ));
                std::fs::rename(&log_path, &quarantine).map_err(|e| CoreError::Internal {
                    context: format!("audit quarantine rename: {e}"),
                })?;
                log::error!(
                    "audit log quarantined ({reason}); preserved at {} — investigate, this \
                     is a tamper or corruption signal",
                    quarantine.display()
                );
                anchor.clear();
                (
                    ChainState {
                        next_seq: 1,
                        last_hmac: ZERO_HMAC_HEX.to_string(),
                    },
                    None,
                    Some(AuditAction::ChainQuarantined { reason }),
                )
            }
        };

        let svc = Self {
            log_path,
            state: Mutex::new(state),
            master_key,
            day_keys: Mutex::new(HashMap::new()),
            anchor,
            startup_breach: Mutex::new(breach),
        };
        if let Some(action) = opening_action {
            svc.record(action)?;
        }
        Ok(svc)
    }

    /// Decide how to open an existing (or absent) log file.
    fn opening_plan(
        log_path: &Path,
        master_key: &Zeroizing<[u8; 32]>,
        loaded_anchor: Option<&super::anchor::AnchorRecord>,
    ) -> Result<OpeningPlan, CoreError> {
        let (entries, malformed) = read_entries_lenient(log_path)?;

        if let Some(bad) = malformed {
            return Ok(OpeningPlan::Quarantine {
                reason: format!(
                    "unparseable line {} ({})",
                    bad.line_number, bad.error
                ),
            });
        }

        if entries.is_empty() {
            // A persisted anchor with no log = the whole file was
            // deleted out from under us.
            let breach = loaded_anchor.map(|a| {
                format!(
                    "audit log missing/empty but anchor records seq {} — file was deleted",
                    a.seq
                )
            });
            return Ok(OpeningPlan::Resume {
                next_seq: 1,
                last_hmac: ZERO_HMAC_HEX.to_string(),
                breach,
            });
        }

        let last = entries.last().expect("non-empty");
        let tail = (last.seq.saturating_add(1), last.hmac.clone());

        // Anchor truncation check applies regardless of scheme.
        let breach = loaded_anchor.and_then(|a| {
            if last.seq < a.seq {
                Some(format!(
                    "audit log tail is seq {} but anchor records seq {} — log was truncated",
                    last.seq, a.seq
                ))
            } else {
                None
            }
        });

        // Already on the per-day scheme (or tampered — verify_chain
        // reports that on read; we keep the file as evidence).
        if Self::entries_verify_per_day(&entries, master_key) {
            return Ok(OpeningPlan::Resume {
                next_seq: tail.0,
                last_hmac: tail.1,
                breach,
            });
        }

        // Intact legacy chain → migrate (archive + fresh chain).
        if Self::entries_verify_legacy(&entries, master_key) {
            return Ok(OpeningPlan::Migrate {
                archived_entries: last.seq,
            });
        }

        // Parseable but verifies under neither scheme: tampered. Keep
        // the file as evidence; verify_chain reports Tampered.
        Ok(OpeningPlan::Resume {
            next_seq: tail.0,
            last_hmac: tail.1,
            breach,
        })
    }

    fn entries_verify_per_day(entries: &[AuditEntry], master: &Zeroizing<[u8; 32]>) -> bool {
        Self::verify_entries(entries, |ts| derive_day_key(master, &day_of(ts)))
            .map(|status| matches!(status, AuditChainStatus::Ok { .. }))
            .unwrap_or(false)
    }

    fn entries_verify_legacy(entries: &[AuditEntry], master: &Zeroizing<[u8; 32]>) -> bool {
        Self::verify_entries(entries, |_| master.clone())
            .map(|status| matches!(status, AuditChainStatus::Ok { .. }))
            .unwrap_or(false)
    }

    /// Path to the audit log file. Exposed for the audit-log UI
    /// endpoint and the tamper-detector.
    pub fn log_path(&self) -> &Path {
        &self.log_path
    }

    /// Anchor persistence state for the audit-status DTO:
    /// `"ok"` or `"degraded"` (anchor writes failing — truncation
    /// detection is impaired until it recovers).
    pub fn anchor_state(&self) -> &'static str {
        self.anchor.state_str()
    }

    fn key_for_day(&self, day: &str) -> Zeroizing<[u8; 32]> {
        let mut cache = self.day_keys.lock().unwrap_or_else(|e| e.into_inner());
        cache
            .entry(day.to_string())
            .or_insert_with(|| derive_day_key(&self.master_key, day))
            .clone()
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
        let day_key = self.key_for_day(&day_of(&timestamp));

        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let seq = state.next_seq;
        let prev_hmac = state.last_hmac.clone();

        let canonical = canonical_input(seq, &timestamp, &action, None, &prev_hmac)?;
        let hmac_bytes = compute_hmac(&day_key, &canonical);
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
        state.last_hmac = hmac_hex.clone();
        drop(state);

        self.anchor.update(seq, hmac_hex);
        Ok(())
    }

    /// Read every parseable entry currently on disk. A malformed tail
    /// no longer turns the whole read into an error — the UI still
    /// renders what's intact, and `verify_chain` reports the tamper.
    pub fn entries(&self) -> Result<Vec<AuditEntry>, CoreError> {
        let (entries, _malformed) = read_entries_lenient(&self.log_path)?;
        Ok(entries)
    }

    /// Recompute the HMAC chain from sequence 1 forward and report
    /// whether every entry verifies. The red-banner UI calls this on
    /// every audit-log fetch. Covers: per-entry HMAC + sequence +
    /// prev-link, unparseable lines (classified as Tampered, never as
    /// an I/O error), and the out-of-band tail anchor (truncation /
    /// deletion detection).
    pub fn verify_chain(&self) -> Result<AuditChainStatus, CoreError> {
        if let Some(reason) = self
            .startup_breach
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        {
            return Ok(AuditChainStatus::Tampered {
                last_valid_sequence: 0,
                reason,
            });
        }

        if !self.log_path.exists() {
            return Ok(AuditChainStatus::Empty);
        }
        let (entries, malformed) = read_entries_lenient(&self.log_path)?;
        if let Some(bad) = malformed {
            return Ok(AuditChainStatus::Tampered {
                last_valid_sequence: entries.last().map(|e| e.seq).unwrap_or(0),
                reason: format!("unparseable line {} — appended garbage", bad.line_number),
            });
        }
        if entries.is_empty() {
            return Ok(AuditChainStatus::Empty);
        }

        // Anchor truncation check before the per-entry walk.
        if let Some(anchor) = self.anchor.current() {
            let last_seq = entries.last().map(|e| e.seq).unwrap_or(0);
            if last_seq < anchor.seq {
                return Ok(AuditChainStatus::Tampered {
                    last_valid_sequence: last_seq,
                    reason: format!(
                        "log tail is seq {last_seq} but anchor records seq {} — truncated",
                        anchor.seq
                    ),
                });
            }
            if let Some(at_anchor) = entries.iter().find(|e| e.seq == anchor.seq) {
                if at_anchor.hmac != anchor.hmac {
                    return Ok(AuditChainStatus::Tampered {
                        last_valid_sequence: 0,
                        reason: format!("entry at anchored seq {} was rewritten", anchor.seq),
                    });
                }
            }
        }

        Self::verify_entries(&entries, |ts| {
            derive_day_key(&self.master_key, &day_of(ts))
        })
    }

    /// Verify the archived legacy chain (`audit.log.v1-archive`),
    /// written before the per-day key migration, under the legacy
    /// single-key scheme. Returns `Empty` when no archive exists.
    pub fn verify_archive(&self) -> Result<AuditChainStatus, CoreError> {
        let archive = self
            .log_path
            .with_file_name(format!("{AUDIT_FILENAME}.v1-archive"));
        if !archive.exists() {
            return Ok(AuditChainStatus::Empty);
        }
        let (entries, malformed) = read_entries_lenient(&archive)?;
        if let Some(bad) = malformed {
            return Ok(AuditChainStatus::Tampered {
                last_valid_sequence: entries.last().map(|e| e.seq).unwrap_or(0),
                reason: format!("archive: unparseable line {}", bad.line_number),
            });
        }
        if entries.is_empty() {
            return Ok(AuditChainStatus::Empty);
        }
        Self::verify_entries(&entries, |_| self.master_key.clone())
    }

    /// Shared chain walk: sequence continuity, prev-link, and per-entry
    /// HMAC under the key selected by `key_for` (per-day for the live
    /// chain, the master key for legacy archives).
    fn verify_entries<F>(entries: &[AuditEntry], key_for: F) -> Result<AuditChainStatus, CoreError>
    where
        F: Fn(&DateTime<Utc>) -> Zeroizing<[u8; 32]>,
    {
        let mut expected_seq: u64 = 1;
        let mut expected_prev: String = ZERO_HMAC_HEX.to_string();
        let mut last_valid: u64 = 0;
        for entry in entries {
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
            let key = key_for(&entry.timestamp);
            let computed = compute_hmac(&key, &canonical);
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

    /// Test-friendly constructor wiring a file-backed secret store in
    /// the same data dir. Production code goes through `new` with the
    /// registry's store.
    #[cfg(test)]
    pub(crate) fn new_for_tests(app_data_dir: PathBuf) -> Result<Self, CoreError> {
        let secrets: Arc<dyn SecretStore> = Arc::new(
            crate::services::EncryptedFileSecretStore::new(app_data_dir.clone()),
        );
        Self::new(app_data_dir, secrets)
    }
}
