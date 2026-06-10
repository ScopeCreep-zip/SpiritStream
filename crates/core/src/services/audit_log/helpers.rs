use std::path::Path;

use chrono::{DateTime, Utc};
use hkdf::Hkdf;
use hmac::Mac;
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::errors::CoreError;

use super::actions::{AuditAction, AuditEntry};
use super::{HmacSha256, HMAC_KEY_INFO};

/// Canonical pre-hash input for a given entry. **Field order matters**
/// — any reorder breaks the chain. JSON serialisation produces stable
/// bytes only when this function controls field order, so we hand-roll
/// the encoding (rather than trusting `serde_json::to_vec`).
pub(super) fn canonical_input(
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

pub(super) fn compute_hmac(key: &[u8; 32], data: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    let result = mac.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

/// Master audit key: HKDF of the machine key under the v1 info string.
/// Per-day chain keys derive from THIS key (see [`derive_day_key`]) —
/// the legacy (pre-day-key) chains used it directly, which is what
/// `verify_legacy_scheme` checks during migration.
pub(super) fn derive_audit_hmac_key(app_data_dir: &Path) -> Result<Zeroizing<[u8; 32]>, CoreError> {
    let machine_key = crate::services::Encryption::get_or_create_machine_key_public(app_data_dir)?;
    let hk = Hkdf::<Sha256>::new(None, &*machine_key);
    let mut out = Zeroizing::new([0u8; 32]);
    hk.expand(HMAC_KEY_INFO, &mut *out)
        .map_err(|e| CoreError::Internal {
            context: format!("audit hkdf: {e}"),
        })?;
    Ok(out)
}

/// Per-day chain key: `HMAC(master, "spiritstream/audit-log/hmac/v1/{day}")`
/// where `day` is the entry timestamp's UTC `YYYY-MM-DD`. Key evolution
/// per epoch (day) is the documented design ("per-day HKDF-derived
/// keys"); deriving from the master (not the machine key directly)
/// keeps the machine key out of this module's steady state.
pub(super) fn derive_day_key(master: &[u8; 32], day: &str) -> Zeroizing<[u8; 32]> {
    let mut info = Vec::with_capacity(HMAC_KEY_INFO.len() + 1 + day.len());
    info.extend_from_slice(HMAC_KEY_INFO);
    info.push(b'/');
    info.extend_from_slice(day.as_bytes());
    Zeroizing::new(compute_hmac(master, &info))
}

/// UTC day bucket for a timestamp — the per-day key selector.
pub(super) fn day_of(timestamp: &DateTime<Utc>) -> String {
    timestamp.format("%Y-%m-%d").to_string()
}

/// A line that failed to parse. An unparseable line in an append-only
/// HMAC-chained file IS tampering (or torn-write corruption) — callers
/// classify it as `Tampered`, never as an I/O error, and startup
/// quarantines rather than bricking.
#[derive(Debug, Clone)]
pub(super) struct MalformedLine {
    pub(super) line_number: usize,
    pub(super) error: String,
}

/// Read the log, returning every parseable entry plus the first
/// malformed line (if any). Only genuine I/O failures return `Err`.
pub(super) fn read_entries_lenient(
    log_path: &Path,
) -> Result<(Vec<AuditEntry>, Option<MalformedLine>), CoreError> {
    if !log_path.exists() {
        return Ok((Vec::new(), None));
    }
    let text = std::fs::read_to_string(log_path).map_err(|e| CoreError::Internal {
        context: format!("audit read: {e}"),
    })?;
    let mut entries = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<AuditEntry>(line) {
            Ok(entry) => entries.push(entry),
            Err(e) => {
                return Ok((
                    entries,
                    Some(MalformedLine {
                        line_number: idx + 1,
                        error: e.to_string(),
                    }),
                ));
            }
        }
    }
    Ok((entries, None))
}
