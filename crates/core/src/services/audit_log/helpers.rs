use std::path::Path;

use chrono::{DateTime, Utc};
use hkdf::Hkdf;
use hmac::Mac;
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::errors::CoreError;

use super::actions::{AuditAction, AuditEntry};
use super::{HmacSha256, HMAC_KEY_INFO, ZERO_HMAC_HEX};

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

pub(super) fn scan_chain_tail(log_path: &Path) -> Result<(u64, String), CoreError> {
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

pub(super) fn read_all_entries(log_path: &Path) -> Result<Vec<AuditEntry>, CoreError> {
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
