//! Encryption Service.
//!
//! # Versioning
//!
//! There are two on-disk encryption versions. Old data is read by the V1
//! path; new data is written by the V2 path. A future migration will
//! bulk-re-encrypt the remaining V1 blobs.
//!
//! * **V1Gcm** — AES-256-GCM with random 96-bit nonces. Legacy
//!   default. Nonce collisions at high volumes (~2^32 records under one
//!   key) compromise confidentiality and authentication, which is why it
//!   is no longer used for new writes.
//! * **V2GcmSiv** — AES-256-GCM-SIV (RFC 8452). Nonce-misuse-resistant
//!   AEAD: nonce reuse only leaks "did these two ciphertexts encrypt the
//!   same plaintext", not the key. New writes use this exclusively.
//!
//! The version is encoded out-of-band:
//! * Profile `.mgs` files carry a 4-byte magic (`MGLA` → V1, `MGL2` → V2)
//!   set by `profile_manager`. The encryption helpers themselves work on
//!   the body after the magic and take an explicit version parameter where
//!   needed.
//! * Stream keys and tokens carry a string prefix (`ENC::` → V1,
//!   `ENC2::` → V2). Dispatch happens inside this module.
//!
//! # Argon2id parameters
//!
//! `derive_key` uses m=64 MiB, t=3, p=4. This matches RFC 9106's
//! memory-constrained recommendation and exceeds OWASP 2025's interactive
//! baseline. Mobile retuning lives behind a separate path.
//!
//! # Layout
//!
//! Each concern lives in its own submodule:
//! - `kdf` — Argon2id key derivation (`derive_key`).
//! - `password` — password-based envelope (`Encryption::encrypt` /
//!   `decrypt_v1` / `decrypt_v2`), used for `.mgs` profile files.
//! - `machine_key` — per-machine key file + stream-key / token /
//!   arbitrary-bytes encryption.
//! - `rotation` — `rotate_machine_key` orchestration with a journaled
//!   pending key (`.stream_key.new`), re-encrypt / rollback, and startup
//!   crash recovery (`recover_interrupted_rotation`).
//! - `rotation_backup` — profile snapshot / restore / retention used by
//!   rotation and its crash recovery.

mod kdf;
mod machine_key;
mod password;
mod rotation;
mod rotation_backup;

#[cfg(test)]
mod rotation_recovery_tests;
#[cfg(test)]
mod tests;

pub use rotation::{RotationRecovery, RotationReport};

use crate::errors::CoreError;

// =========================================================================
// Module-wide constants. All submodules import these via `super::*` /
// `super::KEY_LEN` so a parameter change here propagates everywhere.
// =========================================================================

pub(super) const SALT_LEN: usize = 32;
pub(super) const NONCE_LEN: usize = 12;
pub(super) const KEY_LEN: usize = 32;

/// Minimum password length for profile encryption.
///
/// 12 characters per NIST SP 800-63B-3 and OWASP Authentication Cheat Sheet
/// 2024 guidance for **high-value memorized secrets**. Profile encryption
/// keys protect stream keys, OAuth tokens, the PII blocklist (real name,
/// deadname, hometown), the anonymous-mode salt, and chat-log contents —
/// breach of this key is catastrophic-doxxing-grade for the populations
/// this app serves. The 8-character NIST floor is for low-value secrets
/// paired with a breach-database check that we do not perform.
pub const PROFILE_PASSWORD_MIN_LENGTH: usize = 12;

// Stream key / token prefixes. New writes always emit V2.
pub(super) const STREAM_KEY_PREFIX_V1: &str = "ENC::";
pub(super) const STREAM_KEY_PREFIX_V2: &str = "ENC2::";

pub(super) fn internal<E: std::fmt::Display>(prefix: &str, e: E) -> CoreError {
    CoreError::Internal {
        context: format!("{prefix}: {e}"),
    }
}

/// Encryption service for profile data.
///
/// Unit marker type; the work happens in the `impl` blocks scattered
/// across `password.rs`, `machine_key.rs`, and `rotation.rs`. See module
/// docs for the V1Gcm → V2GcmSiv migration shape.
pub struct Encryption;
