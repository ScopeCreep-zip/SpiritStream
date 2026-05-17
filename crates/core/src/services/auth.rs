//! Authentication failure tracking.
//!
//! Replaces the hardcoded 100ms `tokio::time::sleep()` that previously
//! served as the only brute-force defense on `POST /api/v1/auth/login`.
//! The 100ms sleep was trivially bypassable — an attacker just pipelines
//! requests faster than they're processed.
//!
//! The defense ships in three layers:
//!
//! 1. **Exponential backoff per account** — after each failure, the
//!    caller is required to wait `base_delay × 2^(failures − 1)` capped
//!    at `max_delay` before another attempt is accepted. Single-tenant
//!    installs use one shared "default" account; multi-tenant deploys
//!    (post-rewrite) key by user identifier.
//! 2. **Sliding-window lockout** — once `lockout_threshold` failures
//!    arrive within `lockout_window`, the account is locked for
//!    `lockout_duration`. The lockout is finite (15 min by default) so a
//!    legitimate user who genuinely forgot their token isn't permanently
//!    barred.
//! 3. **Per-IP rate limiting** — sits on top of this service in
//!    the HTTP transport.
//!
//! Note: persistence across server restarts is out-of-scope for this
//! service. The in-memory state is sufficient because the API token
//! itself doesn't survive a restart of the server with a rotated
//! `SPIRITSTREAM_API_TOKEN`. The audit log will record every
//! failure regardless.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Tunable backoff/lockout parameters.
#[derive(Debug, Clone)]
pub struct AuthBackoffConfig {
    /// Initial sleep on the first failure (subsequent attempts double).
    pub base_delay: Duration,
    /// Hard cap on exponential backoff between attempts.
    pub max_delay: Duration,
    /// Failure count within `lockout_window` that triggers a lockout.
    pub lockout_threshold: u32,
    /// Sliding window for counting failures toward `lockout_threshold`.
    pub lockout_window: Duration,
    /// How long an account stays locked after the threshold is breached.
    pub lockout_duration: Duration,
}

impl Default for AuthBackoffConfig {
    fn default() -> Self {
        Self {
            base_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(30),
            lockout_threshold: 10,
            lockout_window: Duration::from_secs(3600), // 1 hour
            lockout_duration: Duration::from_secs(15 * 60), // 15 minutes
        }
    }
}

#[derive(Debug, Default)]
struct AccountState {
    /// Number of consecutive failures since the last successful login.
    /// Resets to zero on success. Drives the exponential-backoff sleep.
    consecutive_failures: u32,
    /// Timestamps of failures within the lockout sliding window. Older
    /// entries are evicted lazily when `record_failure` runs.
    window_failures: Vec<Instant>,
    /// `Some(unlock_at)` while a lockout is in effect; `None` otherwise.
    locked_until: Option<Instant>,
}

/// Brute-force defense for `POST /api/v1/auth/login`.
///
/// Construct once (via `AuthService::new()` or `AuthService::with_config`)
/// and share across HTTP and CLI transports through `ServiceRegistry`.
pub struct AuthService {
    accounts: Mutex<HashMap<String, AccountState>>,
    config: AuthBackoffConfig,
}

impl AuthService {
    pub fn new() -> Self {
        Self::with_config(AuthBackoffConfig::default())
    }

    pub fn with_config(config: AuthBackoffConfig) -> Self {
        Self {
            accounts: Mutex::new(HashMap::new()),
            config,
        }
    }

    pub fn config(&self) -> &AuthBackoffConfig {
        &self.config
    }

    /// Check whether `account` is currently locked. Returns the remaining
    /// lockout duration when locked, `None` otherwise. Callers should
    /// surface the duration as a `Retry-After` header so well-behaved
    /// clients back off.
    pub fn check_locked(&self, account: &str) -> Option<Duration> {
        let mut accounts = self.accounts.lock().unwrap_or_else(|e| e.into_inner());
        let state = accounts.entry(account.to_string()).or_default();
        clear_expired_lock(state);
        state
            .locked_until
            .map(|t| t.saturating_duration_since(Instant::now()))
    }

    /// Compute the backoff that should be applied **before** the next
    /// attempt is allowed, given the current failure history.
    pub fn current_backoff(&self, account: &str) -> Duration {
        let accounts = self.accounts.lock().unwrap_or_else(|e| e.into_inner());
        let Some(state) = accounts.get(account) else {
            return Duration::ZERO;
        };
        backoff_for_failures(state.consecutive_failures, &self.config)
    }

    /// Record a failed login attempt. Returns:
    /// * `Err(remaining)` if recording this failure pushed the account
    ///   into a fresh lockout or extended an existing one — the caller
    ///   should reject the request with 429 and surface `remaining` as
    ///   the `Retry-After` value.
    /// * `Ok(backoff)` otherwise — caller should sleep for `backoff`
    ///   before responding so timing-based bypass attempts can't go
    ///   faster than the policy allows.
    pub fn record_failure(&self, account: &str) -> Result<Duration, Duration> {
        let mut accounts = self.accounts.lock().unwrap_or_else(|e| e.into_inner());
        let state = accounts.entry(account.to_string()).or_default();
        clear_expired_lock(state);

        // If the account is already locked, refuse to count this attempt
        // (so an attacker can't extend the lockout indefinitely). Just
        // report the remaining time.
        if let Some(unlock_at) = state.locked_until {
            return Err(unlock_at.saturating_duration_since(Instant::now()));
        }

        let now = Instant::now();
        state.consecutive_failures = state.consecutive_failures.saturating_add(1);
        state.window_failures.push(now);

        // Evict any failures older than the sliding window.
        let cutoff = now.checked_sub(self.config.lockout_window);
        if let Some(cutoff) = cutoff {
            state.window_failures.retain(|t| *t >= cutoff);
        }

        // Threshold check.
        if state.window_failures.len() as u32 >= self.config.lockout_threshold {
            let unlock_at = now + self.config.lockout_duration;
            state.locked_until = Some(unlock_at);
            return Err(self.config.lockout_duration);
        }

        Ok(backoff_for_failures(
            state.consecutive_failures,
            &self.config,
        ))
    }

    /// Clear failure state for an account after a successful login.
    pub fn record_success(&self, account: &str) {
        let mut accounts = self.accounts.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(state) = accounts.get_mut(account) {
            state.consecutive_failures = 0;
            state.window_failures.clear();
            state.locked_until = None;
        }
    }
}

impl Default for AuthService {
    fn default() -> Self {
        Self::new()
    }
}

fn clear_expired_lock(state: &mut AccountState) {
    if let Some(unlock_at) = state.locked_until {
        if Instant::now() >= unlock_at {
            state.locked_until = None;
            state.window_failures.clear();
            state.consecutive_failures = 0;
        }
    }
}

fn backoff_for_failures(failures: u32, cfg: &AuthBackoffConfig) -> Duration {
    if failures == 0 {
        return Duration::ZERO;
    }
    // base * 2^(failures - 1), saturating before the cap so we don't
    // overflow Duration on extreme failure counts.
    let exponent = failures.saturating_sub(1).min(30);
    let multiplier = 1u64.checked_shl(exponent).unwrap_or(u64::MAX);
    let scaled = cfg.base_delay.saturating_mul(multiplier as u32);
    scaled.min(cfg.max_delay)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fast_config() -> AuthBackoffConfig {
        AuthBackoffConfig {
            base_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(500),
            lockout_threshold: 5,
            lockout_window: Duration::from_secs(60),
            lockout_duration: Duration::from_millis(100),
        }
    }

    #[test]
    fn fresh_account_has_no_backoff_and_no_lock() {
        let svc = AuthService::with_config(fast_config());
        assert_eq!(svc.current_backoff("alice"), Duration::ZERO);
        assert_eq!(svc.check_locked("alice"), None);
    }

    #[test]
    fn backoff_doubles_with_each_failure_until_capped() {
        let svc = AuthService::with_config(fast_config());
        let first = svc.record_failure("alice").expect("not locked");
        assert_eq!(first, Duration::from_millis(10));
        let second = svc.record_failure("alice").expect("not locked");
        assert_eq!(second, Duration::from_millis(20));
        let third = svc.record_failure("alice").expect("not locked");
        assert_eq!(third, Duration::from_millis(40));
        let fourth = svc.record_failure("alice").expect("not locked");
        assert_eq!(fourth, Duration::from_millis(80));
        // The 5th failure trips the lockout threshold and returns Err.
        let lockout_remaining = svc
            .record_failure("alice")
            .expect_err("5 failures must lock the account");
        // Lockout window is 100ms in test config; allow a small slack.
        assert!(lockout_remaining >= Duration::from_millis(50));
    }

    #[test]
    fn lockout_blocks_further_attempts_with_remaining_time() {
        let svc = AuthService::with_config(fast_config());
        for _ in 0..5 {
            let _ = svc.record_failure("alice");
        }
        // Now locked — subsequent attempts return Err with remaining time.
        let remaining = svc
            .check_locked("alice")
            .expect("account is locked after 5 failures");
        assert!(remaining > Duration::ZERO);
        assert!(remaining <= Duration::from_millis(100));
    }

    #[test]
    fn lockout_clears_after_duration_elapses() {
        let svc = AuthService::with_config(fast_config());
        for _ in 0..5 {
            let _ = svc.record_failure("alice");
        }
        assert!(svc.check_locked("alice").is_some());
        std::thread::sleep(Duration::from_millis(150));
        assert!(
            svc.check_locked("alice").is_none(),
            "lockout must clear after duration"
        );
        // After unlock, the account is fresh again.
        assert_eq!(svc.current_backoff("alice"), Duration::ZERO);
    }

    #[test]
    fn success_resets_consecutive_failures() {
        let svc = AuthService::with_config(fast_config());
        let _ = svc.record_failure("alice");
        let _ = svc.record_failure("alice");
        svc.record_success("alice");
        assert_eq!(svc.current_backoff("alice"), Duration::ZERO);
    }

    #[test]
    fn separate_accounts_have_independent_state() {
        let svc = AuthService::with_config(fast_config());
        for _ in 0..5 {
            let _ = svc.record_failure("alice");
        }
        assert!(svc.check_locked("alice").is_some());
        assert!(
            svc.check_locked("bob").is_none(),
            "bob must not be locked when alice is"
        );
        assert_eq!(svc.current_backoff("bob"), Duration::ZERO);
    }

    #[test]
    fn locked_account_rejects_further_failures_without_extending() {
        let svc = AuthService::with_config(fast_config());
        for _ in 0..5 {
            let _ = svc.record_failure("alice");
        }
        // A new attempt while locked should NOT push the unlock time out
        // further. Capture remaining, attempt once, and confirm the new
        // remaining is not greater than the old one.
        let remaining_before = svc.check_locked("alice").unwrap();
        let _ = svc.record_failure("alice");
        let remaining_after = svc.check_locked("alice").unwrap();
        // Allow strict <= because of monotonic clock drift during the call.
        assert!(remaining_after <= remaining_before);
    }
}
