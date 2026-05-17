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
//!
//! The CLI does not use this service; it has a `--confirm` flag that
//! the transport-cli layer reads directly.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Default lifetime for a confirmation token. Long enough for a human
/// to read a confirmation dialog and click "yes", short enough that an
/// idle window won't accumulate stale auth.
pub const CONFIRM_TOKEN_TTL_SECS: u64 = 30;

/// Issues and validates one-shot confirmation tokens.
pub struct ConfirmTokenService {
    tokens: Mutex<HashMap<TokenKey, Instant>>,
    ttl: Duration,
}

type TokenKey = (String, String); // (intent, token)

impl ConfirmTokenService {
    pub fn new() -> Self {
        Self::with_ttl(Duration::from_secs(CONFIRM_TOKEN_TTL_SECS))
    }

    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            tokens: Mutex::new(HashMap::new()),
            ttl,
        }
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Issue a fresh token scoped to `intent`. The token is returned to
    /// the caller and stored internally; expires after [`Self::ttl`].
    pub fn issue(&self, intent: &str) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        let mut store = self.tokens.lock().unwrap_or_else(|e| e.into_inner());
        self.gc(&mut store);
        store.insert((intent.to_string(), token.clone()), Instant::now());
        token
    }

    /// Validate and consume a token. Returns `true` if a matching
    /// non-expired entry existed (and removes it); `false` otherwise.
    pub fn consume(&self, intent: &str, token: &str) -> bool {
        let mut store = self.tokens.lock().unwrap_or_else(|e| e.into_inner());
        self.gc(&mut store);
        let key = (intent.to_string(), token.to_string());
        match store.remove(&key) {
            Some(issued_at) => issued_at.elapsed() <= self.ttl,
            None => false,
        }
    }

    fn gc(&self, store: &mut HashMap<TokenKey, Instant>) {
        let ttl = self.ttl;
        store.retain(|_, issued_at| issued_at.elapsed() <= ttl);
    }
}

impl Default for ConfirmTokenService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issued_token_consumes_within_ttl() {
        let svc = ConfirmTokenService::new();
        let token = svc.issue("clear_data");
        assert!(svc.consume("clear_data", &token));
    }

    #[test]
    fn token_is_one_shot() {
        let svc = ConfirmTokenService::new();
        let token = svc.issue("rotate_machine_key");
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
        let svc = ConfirmTokenService::new();
        let token = svc.issue("clear_data");
        assert!(!svc.consume("rotate_machine_key", &token));
    }

    #[test]
    fn token_expires_after_ttl() {
        let svc = ConfirmTokenService::with_ttl(Duration::from_millis(50));
        let token = svc.issue("clear_data");
        std::thread::sleep(Duration::from_millis(120));
        assert!(
            !svc.consume("clear_data", &token),
            "expired token must not validate"
        );
    }

    #[test]
    fn unknown_token_is_rejected() {
        let svc = ConfirmTokenService::new();
        assert!(!svc.consume("clear_data", "totally-bogus-token"));
    }

    #[test]
    fn each_issue_produces_unique_token() {
        let svc = ConfirmTokenService::new();
        let t1 = svc.issue("x");
        let t2 = svc.issue("x");
        assert_ne!(t1, t2, "tokens must be unique even for the same intent");
    }
}
