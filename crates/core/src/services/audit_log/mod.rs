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
//!
//! # Layout
//!
//! - `actions`: wire types (`AuditEntry`, `AuditAction`, `AuditChainStatus`).
//! - `helpers`: canonical HMAC input, HKDF key derivation, log-file I/O.
//! - `service`: `AuditLogService` + `record` / `verify_chain`.

mod actions;
mod helpers;
mod service;

#[cfg(test)]
mod tests;

pub use actions::{AuditAction, AuditChainStatus, AuditEntry};
pub use service::AuditLogService;

pub(super) const AUDIT_DIRNAME: &str = "audit";
pub(super) const AUDIT_FILENAME: &str = "audit.log";
pub(super) const HMAC_KEY_INFO: &[u8] = b"spiritstream/audit-log/hmac/v1";
pub(super) const ZERO_HMAC_HEX: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

pub(super) type HmacSha256 = hmac::Hmac<sha2::Sha256>;
