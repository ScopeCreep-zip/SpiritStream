//! Phase-4 regression tests: tail-anchor truncation/deletion detection,
//! per-day key evolution, legacy-chain migration, and quarantine of
//! unparseable logs. These pin the gaps the original chain design left
//! open: tail truncation was undetected, "per-day keys" were documented
//! but not implemented, and one garbage line both hid tampering behind
//! a 500 and bricked startup.

use std::sync::Arc;

use tempfile::TempDir;

use crate::services::EncryptedFileSecretStore;
use crate::traits::SecretStore;

use super::actions::{AuditAction, AuditChainStatus};
use super::service::AuditLogService;

fn store_for(dir: &TempDir) -> Arc<dyn SecretStore> {
    Arc::new(EncryptedFileSecretStore::new(dir.path().to_path_buf()))
}

fn svc(dir: &TempDir) -> AuditLogService {
    AuditLogService::new(dir.path().to_path_buf(), store_for(dir)).unwrap()
}

fn log_path(dir: &TempDir) -> std::path::PathBuf {
    dir.path().join("audit").join("audit.log")
}

/// Wait until the async anchor writer has flushed (bounded poll on the
/// secret-store key existing with the expected seq).
fn settle_anchor(dir: &TempDir, expected_seq: u64) {
    let store = EncryptedFileSecretStore::new(dir.path().to_path_buf());
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    for _ in 0..100 {
        if let Ok(Some(bytes)) = rt.block_on(store.get("audit", "chain-anchor")) {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                if value.get("seq").and_then(|s| s.as_u64()) == Some(expected_seq) {
                    return;
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("anchor never reached seq {expected_seq}");
}

/// THE truncation regression: delete the last lines of the log, restart
/// — the pre-fix scan resumed from the truncated tail with zero tamper
/// signal. The anchor must flag it.
#[test]
fn tail_truncation_is_detected_after_restart() {
    let dir = TempDir::new().unwrap();
    {
        let s = svc(&dir);
        for _ in 0..3 {
            s.record(AuditAction::AppStarted).unwrap();
        }
    }
    settle_anchor(&dir, 3);

    // Attacker deletes the last entry (e.g. the PanicTriggered evidence).
    let text = std::fs::read_to_string(log_path(&dir)).unwrap();
    let kept: Vec<&str> = text.lines().take(2).collect();
    std::fs::write(log_path(&dir), format!("{}\n", kept.join("\n"))).unwrap();

    let s = svc(&dir);
    let status = s.verify_chain().unwrap();
    let AuditChainStatus::Tampered { reason, .. } = status else {
        panic!("truncation must be reported as Tampered, got {status:?}");
    };
    assert!(reason.contains("truncated"), "reason: {reason}");
}

/// Whole-file deletion: previously reported as a clean `Empty` status.
#[test]
fn whole_file_deletion_is_detected() {
    let dir = TempDir::new().unwrap();
    {
        let s = svc(&dir);
        s.record(AuditAction::AppStarted).unwrap();
    }
    settle_anchor(&dir, 1);
    std::fs::remove_file(log_path(&dir)).unwrap();

    let s = svc(&dir);
    let status = s.verify_chain().unwrap();
    assert!(
        matches!(status, AuditChainStatus::Tampered { .. }),
        "deleted log must be Tampered, got {status:?}"
    );
}

/// A garbage line is tampering, not an I/O error — and it must never
/// brick startup. The corrupt file is quarantined, a fresh chain starts
/// with a `ChainQuarantined` entry, and recording continues.
#[test]
fn malformed_line_quarantines_instead_of_bricking() {
    let dir = TempDir::new().unwrap();
    {
        let s = svc(&dir);
        s.record(AuditAction::AppStarted).unwrap();
    }
    settle_anchor(&dir, 1);
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(log_path(&dir))
            .unwrap();
        writeln!(f, "{{ definitely not an audit entry").unwrap();
    }

    // Pre-fix: AuditLogService::new errored here and the whole backend
    // refused to start.
    let s = svc(&dir);
    let entries = s.entries().unwrap();
    assert!(
        entries
            .iter()
            .any(|e| matches!(e.action, AuditAction::ChainQuarantined { .. })),
        "fresh chain must record the quarantine"
    );
    // The quarantined file is preserved as evidence.
    let quarantined: Vec<_> = std::fs::read_dir(dir.path().join("audit"))
        .unwrap()
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .contains("quarantined")
        })
        .collect();
    assert_eq!(quarantined.len(), 1, "corrupt log must be preserved");
    // And the new chain is healthy + writable.
    s.record(AuditAction::AppStarted).unwrap();
    assert!(matches!(
        s.verify_chain().unwrap(),
        AuditChainStatus::Ok { .. }
    ));
}

/// A garbage line appended mid-session reports Tampered on verify (not
/// a 500-shaped Err) without losing the parseable entries.
#[test]
fn malformed_line_mid_session_reports_tampered() {
    let dir = TempDir::new().unwrap();
    let s = svc(&dir);
    s.record(AuditAction::AppStarted).unwrap();
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(log_path(&dir))
            .unwrap();
        writeln!(f, "garbage").unwrap();
    }
    let status = s.verify_chain().unwrap();
    let AuditChainStatus::Tampered {
        last_valid_sequence,
        reason,
    } = status
    else {
        panic!("appended garbage must be Tampered");
    };
    assert_eq!(last_valid_sequence, 1);
    assert!(reason.contains("unparseable"), "reason: {reason}");
    // Entries remain readable for the UI.
    assert_eq!(s.entries().unwrap().len(), 1);
}

/// Per-day key evolution: entries on different UTC days verify under
/// their own day keys, including across a midnight boundary.
#[test]
fn per_day_keys_verify_across_midnight() {
    use chrono::TimeZone;
    let dir = TempDir::new().unwrap();
    let s = svc(&dir);
    let day1 = chrono::Utc
        .with_ymd_and_hms(2026, 6, 9, 23, 59, 30)
        .unwrap();
    let day2 = chrono::Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 30).unwrap();
    s.record_with_timestamp(AuditAction::AppStarted, day1).unwrap();
    s.record_with_timestamp(AuditAction::AppStopped, day2).unwrap();
    assert!(matches!(
        s.verify_chain().unwrap(),
        AuditChainStatus::Ok {
            entries_verified: 2
        }
    ));

    // Two distinct days produced two distinct keys: swapping an entry's
    // timestamp to the other day must break verification.
    let text = std::fs::read_to_string(log_path(&dir)).unwrap();
    let swapped = text.replace("2026-06-09", "2026-06-10");
    assert_ne!(text, swapped, "fixture must actually contain day1");
    std::fs::write(log_path(&dir), swapped).unwrap();
    assert!(matches!(
        s.verify_chain().unwrap(),
        AuditChainStatus::Tampered { .. }
    ));
}

/// Legacy single-key chains migrate: archived intact, fresh chain
/// starts with `ChainMigrated`, and `verify_archive` keeps the history
/// checkable.
#[test]
fn legacy_chain_is_archived_and_stays_verifiable() {
    use super::helpers::{canonical_input, compute_hmac, derive_audit_hmac_key};
    use super::ZERO_HMAC_HEX;

    let dir = TempDir::new().unwrap();
    // Hand-write a 2-entry chain under the LEGACY scheme (master key
    // used directly).
    let master = derive_audit_hmac_key(dir.path()).unwrap();
    std::fs::create_dir_all(dir.path().join("audit")).unwrap();
    let mut prev = ZERO_HMAC_HEX.to_string();
    let mut lines = String::new();
    for seq in 1..=2u64 {
        let ts = chrono::Utc::now();
        let action = AuditAction::AppStarted;
        let canonical = canonical_input(seq, &ts, &action, None, &prev).unwrap();
        let hmac = hex::encode(compute_hmac(&master, &canonical));
        let entry = serde_json::json!({
            "seq": seq,
            "timestamp": ts.to_rfc3339(),
            "action": {"kind": "app_started"},
            "prevHmac": prev,
            "hmac": hmac,
        });
        lines.push_str(&format!("{entry}\n"));
        prev = hmac;
    }
    std::fs::write(log_path(&dir), lines).unwrap();

    let s = svc(&dir);
    // Migration archived the legacy chain…
    assert!(dir
        .path()
        .join("audit")
        .join("audit.log.v1-archive")
        .exists());
    // …recorded the migration on the fresh chain…
    assert!(s.entries().unwrap().iter().any(
        |e| matches!(e.action, AuditAction::ChainMigrated { archived_entries: 2 })
    ));
    // …and both chains verify.
    assert!(matches!(
        s.verify_chain().unwrap(),
        AuditChainStatus::Ok { .. }
    ));
    assert!(matches!(
        s.verify_archive().unwrap(),
        AuditChainStatus::Ok {
            entries_verified: 2
        }
    ));
}

/// Anchor lag honesty: entries appended after the last anchor write
/// verify by chain alone, so a slightly-behind anchor is never a false
/// tamper alarm.
#[test]
fn anchor_lag_is_not_a_false_positive() {
    let dir = TempDir::new().unwrap();
    {
        let s = svc(&dir);
        s.record(AuditAction::AppStarted).unwrap();
    }
    settle_anchor(&dir, 1);
    // Restart and append more — anchor now lags behind tail until the
    // writer catches up; verification must still be Ok.
    let s = svc(&dir);
    s.record(AuditAction::AppStopped).unwrap();
    assert!(matches!(
        s.verify_chain().unwrap(),
        AuditChainStatus::Ok {
            entries_verified: 2
        }
    ));
}
