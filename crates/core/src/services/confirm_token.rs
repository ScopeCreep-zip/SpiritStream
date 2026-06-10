//! Confirmation tokens for destructive operations.
//!
//! Three endpoints are gated by this service:
//! `DELETE /api/v1/settings/data`, `POST /api/v1/security/machine-key/rotate`,
//! and `POST /api/v1/security/sessions/revoke-all`. Each requires the
//! caller to first request a token from
//! `POST /api/v1/security/confirm-token { intent }`, then pass it as the
//! `X-Confirm-Token` header on the destructive call.
//!
//! Tokens are:
//! - Scoped to a specific `intent` so a `clear_data` confirmation can
//!   never be reused as a `rotate_machine_key` confirmation.
//! - One-shot — consumed on success.
//! - Short-lived — default 30-second TTL.
//! - **Cross-process** — stored hashed in
//!   `DATA_DIR/run/confirm_tokens.json` (see
//!   [`crate::services::TokenFileStore`]). A token issued by
//!   `spiritstream-cli confirm-token issue` genuinely IS consumable by
//!   the running HTTP server's destructive endpoints; the previous
//!   in-memory design made that claim a lie (the token died with the
//!   one-shot CLI process).

use std::path::Path;
use std::time::Duration;

use crate::errors::CoreError;
use crate::services::TokenFileStore;

/// Default lifetime for a confirmation token. Long enough for a human
/// to read a confirmation dialog and click "yes", short enough that an
/// idle window won't accumulate stale auth.
pub const CONFIRM_TOKEN_TTL_SECS: u64 = 30;

/// Issues and validates one-shot confirmation tokens.
pub struct ConfirmTokenService {
    store: TokenFileStore,
    ttl: Duration,
}

impl ConfirmTokenService {
    pub fn new(data_dir: &Path) -> Self {
        Self::with_ttl(data_dir, Duration::from_secs(CONFIRM_TOKEN_TTL_SECS))
    }

    pub fn with_ttl(data_dir: &Path, ttl: Duration) -> Self {
        Self {
            store: TokenFileStore::new(data_dir, "confirm_tokens", ttl),
            ttl,
        }
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Issue a fresh token scoped to `intent`. The token is returned to
    /// the caller and persisted (hashed); expires after [`Self::ttl`].
    pub fn issue(&self, intent: &str) -> Result<String, CoreError> {
        let token = uuid::Uuid::new_v4().to_string();
        self.store.insert(&[intent, &token])?;
        Ok(token)
    }

    /// Validate and consume a token. Returns `true` if a matching
    /// non-expired entry existed (and removes it); `false` otherwise.
    /// A store failure fails CLOSED (loudly) — a destructive op must
    /// never proceed on an unreadable confirmation state.
    pub fn consume(&self, intent: &str, token: &str) -> bool {
        match self.store.consume(&[intent, token]) {
            Ok(valid) => valid,
            Err(e) => {
                log::error!("confirm-token store unreadable — refusing confirmation: {e}");
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn svc(dir: &TempDir) -> ConfirmTokenService {
        ConfirmTokenService::new(dir.path())
    }

    #[test]
    fn issued_token_consumes_within_ttl() {
        let dir = TempDir::new().unwrap();
        let svc = svc(&dir);
        let token = svc.issue("clear_data").unwrap();
        assert!(svc.consume("clear_data", &token));
    }

    #[test]
    fn token_is_one_shot() {
        let dir = TempDir::new().unwrap();
        let svc = svc(&dir);
        let token = svc.issue("rotate_machine_key").unwrap();
        assert!(svc.consume("rotate_machine_key", &token));
        assert!(
            !svc.consume("rotate_machine_key", &token),
            "second consume must fail"
        );
    }

    #[test]
    fn intent_mismatch_is_rejected() {
        // Token issued for one intent must not work on another, even
        // within TTL. Prevents a CSRF-shaped "use a leaked clear_data
        // token to rotate the machine key" attack.
        let dir = TempDir::new().unwrap();
        let svc = svc(&dir);
        let token = svc.issue("clear_data").unwrap();
        assert!(!svc.consume("rotate_machine_key", &token));
    }

    #[test]
    fn token_expires_after_ttl() {
        let dir = TempDir::new().unwrap();
        let svc = ConfirmTokenService::with_ttl(dir.path(), Duration::from_millis(50));
        let token = svc.issue("clear_data").unwrap();
        std::thread::sleep(Duration::from_millis(120));
        assert!(
            !svc.consume("clear_data", &token),
            "expired token must not validate"
        );
    }

    #[test]
    fn unknown_token_is_rejected() {
        let dir = TempDir::new().unwrap();
        let svc = svc(&dir);
        assert!(!svc.consume("clear_data", "totally-bogus-token"));
    }

    #[test]
    fn each_issue_produces_unique_token() {
        let dir = TempDir::new().unwrap();
        let svc = svc(&dir);
        let t1 = svc.issue("x").unwrap();
        let t2 = svc.issue("x").unwrap();
        assert_ne!(t1, t2, "tokens must be unique even for the same intent");
    }

    /// THE cross-process property the Q6 comments used to falsely
    /// claim: a token issued by one service instance (≈ the CLI
    /// process) is consumable by another (≈ the HTTP server).
    #[test]
    fn token_issued_by_one_instance_consumes_in_another() {
        let dir = TempDir::new().unwrap();
        let cli = ConfirmTokenService::new(dir.path());
        let server = ConfirmTokenService::new(dir.path());
        let token = cli.issue("clear_data").unwrap();
        assert!(server.consume("clear_data", &token));
        assert!(!cli.consume("clear_data", &token), "still one-shot");
    }
}
