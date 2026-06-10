use tempfile::TempDir;

use super::actions::{AuditAction, AuditChainStatus};
use super::service::AuditLogService;
use super::ZERO_HMAC_HEX;

fn svc() -> (TempDir, AuditLogService) {
    let dir = TempDir::new().unwrap();
    let svc = AuditLogService::new_for_tests(dir.path().to_path_buf()).unwrap();
    (dir, svc)
}

#[test]
fn append_then_read_roundtrips_and_carries_seq_plus_hmac() {
    let (_dir, svc) = svc();
    svc.record(AuditAction::AppStarted).unwrap();
    svc.record(AuditAction::PanicTriggered {
        streams_stopped: 2,
        elapsed_ms: 145,
        connector_errors: Vec::new(),
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
    let svc = AuditLogService::new_for_tests(dir.path().to_path_buf()).unwrap();
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
    let svc = AuditLogService::new_for_tests(dir.path().to_path_buf()).unwrap();
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

/// G2: SessionRevoked is one of the four "destructive ops" the plan
/// requires the chain to surface. Pre-G2 the variant didn't exist and
/// `revoke-all-sessions` wrote no audit breadcrumb — post-incident review
/// couldn't reconstruct "user demanded all sessions out at T."
#[test]
fn session_revoked_entry_lands_in_chain() {
    let (_dir, svc) = svc();
    svc.record(AuditAction::SessionRevoked { count: 7 })
        .unwrap();
    let entries = svc.entries().unwrap();
    assert_eq!(entries.len(), 1);
    assert!(matches!(
        entries[0].action,
        AuditAction::SessionRevoked { count: 7 }
    ));
    // Chain verifies — adding a new variant must not break the HMAC.
    assert!(matches!(
        svc.verify_chain().unwrap(),
        AuditChainStatus::Ok {
            entries_verified: 1
        }
    ));
}

/// G2: ConfirmTokenIssued records the INTENT but never the token value.
/// The chain entry is the proof that the user asked for X around time T,
/// independent of whether they followed through with the destructive
/// op (which gets its own subsequent entry — pair via wall-clock).
#[test]
fn confirm_token_issued_records_intent_not_token() {
    let (_dir, svc) = svc();
    svc.record(AuditAction::ConfirmTokenIssued {
        intent: "rotate_machine_key".into(),
    })
    .unwrap();
    let entries = svc.entries().unwrap();
    assert_eq!(entries.len(), 1);
    match &entries[0].action {
        AuditAction::ConfirmTokenIssued { intent } => {
            assert_eq!(intent, "rotate_machine_key");
        }
        other => panic!("expected ConfirmTokenIssued, got {other:?}"),
    }
    // Sanity: the raw JSON line carries the intent but no `token` field.
    let line = std::fs::read_to_string(svc.log_path()).unwrap();
    assert!(line.contains("rotate_machine_key"));
    assert!(
        !line.contains("\"token\":"),
        "ConfirmTokenIssued must not record the token value, got: {line}",
    );
    assert!(matches!(
        svc.verify_chain().unwrap(),
        AuditChainStatus::Ok {
            entries_verified: 1
        }
    ));
}

#[test]
fn pii_filter_entry_records_phrase_id_not_phrase_text() {
    let (_dir, svc) = svc();
    svc.record(AuditAction::ChatMessagePiiBlocked {
        platforms: vec!["twitch".into()],
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
        let svc = AuditLogService::new_for_tests(dir.path().to_path_buf()).unwrap();
        svc.record(AuditAction::AppStarted).unwrap();
    }
    // Re-open — should pick up seq=2 + the prior hmac.
    let svc = AuditLogService::new_for_tests(dir.path().to_path_buf()).unwrap();
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
