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
//! # Keys
//!
//! A master key derives from the machine key via
//! `HKDF-SHA256(machine_key, info="spiritstream/audit-log/hmac/v1")`;
//! each entry is then HMAC'd under a per-day key
//! `HMAC(master, info + "/" + YYYY-MM-DD)` selected by the entry's UTC
//! timestamp (key evolution per day). No key appears on disk; both are
//! regenerated identically on every start, so a process restart resumes
//! the chain. Chains written before the per-day scheme verified under
//! the master key directly; on first open such a chain is archived to
//! `audit.log.v1-archive` (still checkable via
//! [`AuditLogService::verify_archive`]) and a fresh chain begins with a
//! `ChainMigrated` entry.
//!
//! # Tamper detection
//!
//! [`AuditLogService::verify_chain`] walks the file from sequence 1
//! forward and recomputes each entry's HMAC; any modified, reordered,
//! or mid-file-deleted line reports `Tampered` with the last valid
//! sequence. Two attacks the chain alone cannot catch are covered
//! separately:
//!
//! * **Tail truncation / file deletion** — the latest `(seq, hmac)`
//!   pair is anchored in the [`crate::traits::SecretStore`] (OS keyring
//!   or encrypted file store) after every append. A log whose tail sits
//!   below the anchor reports `Tampered`. The anchor write is async, so
//!   it can lag the log by a few entries after a crash — entries past
//!   the anchor verify by chain alone, and that lag window is the
//!   documented honest limit of truncation detection.
//! * **Appended garbage / torn writes** — an unparseable line is
//!   classified as `Tampered` (never as an I/O error). At startup a
//!   corrupt log is quarantined to `audit.log.quarantined-<ts>` and a
//!   fresh chain begins with a `ChainQuarantined` entry, so a corrupt
//!   file can neither hide behind a 500 nor brick the backend.
//!
//! Scope honesty: the chain DETECTS tampering, it does not prevent it,
//! and an attacker who can read `.stream_key` (same-user file access)
//! can re-sign a rewritten chain — though the keyring-backed anchor
//! still flags truncation in that scenario on platforms where the
//! keyring is not a same-directory file.
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
mod anchor;
mod helpers;
mod service;

#[cfg(test)]
mod integrity_tests;
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
