use std::env;
use std::num::NonZeroU32;

use governor::{clock::DefaultClock, state::keyed::DefaultKeyedStateStore, Quota, RateLimiter};

use crate::DEFAULT_RATE_LIMIT_PER_MINUTE;

/// Per-endpoint rate limiter keyed by auth-subject (or peer IP
/// when no auth is available). One per high-risk route plus a `default_auth`
/// catch-all for everything else.
pub(crate) type KeyedLimiter = RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>;

/// Per-endpoint rate limit configuration.
///
/// Each high-risk endpoint has its own keyed rate limiter. The `default`
/// catch-all replaces the prior single global governor quota. Burst sizes
/// follow the plan's recommended initial values; quotas can be tuned with
/// telemetry once observability lands.
pub(crate) struct EndpointRateLimiters {
    /// `POST /api/v1/auth/login` — 5/min keyed on peer IP. Per-account
    /// exponential backoff lives in `AuthService`.
    pub login: KeyedLimiter,
    /// `POST /api/v1/chat/messages` — 20/min sustained, burst 5,
    /// keyed on auth subject.
    pub chat_send: KeyedLimiter,
    /// `POST /api/v1/streams` — 10/min sustained, burst 2, keyed on
    /// auth subject. FFmpeg spawn is expensive.
    pub stream_start: KeyedLimiter,
    /// `POST /api/v1/oauth/*/flow` — 5/min keyed on auth subject.
    pub oauth_flow: KeyedLimiter,
    /// `POST /api/v1/security/confirm-token` — 5/min keyed on auth
    /// subject. Each destructive intent (clear_data, rotate_machine_key,
    /// enable_facebook_chat, …) gates on a freshly-issued token, so the
    /// legitimate user only needs a handful per minute. Tight bucket
    /// prevents an attacker who steals a token from flooding the
    /// confirm-token endpoint with noise to drown out the legit user's
    /// next destructive op behind a 429.
    pub confirm_token: KeyedLimiter,
    /// `POST /api/v1/security/sessions/revoke-all` — 3/min keyed on
    /// auth subject. Emergency "log everyone out" — should never need
    /// more than a few per minute. Keeps an attacker who steals a
    /// session from blocking the user's panic-revoke.
    pub revoke_sessions: KeyedLimiter,
    /// `POST /api/v1/security/machine-key/rotate` — 2/min keyed on
    /// auth subject. Extremely rare destructive op; tight bucket.
    pub machine_key_rotate: KeyedLimiter,
    /// Catch-all for every other authenticated request. Replaces the
    /// prior single global `NotKeyed` quota.
    pub default_auth: KeyedLimiter,
}

impl EndpointRateLimiters {
    pub(crate) fn from_env() -> Self {
        let default_quota = env::var("SPIRITSTREAM_RATE_LIMIT")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .and_then(NonZeroU32::new)
            .unwrap_or_else(|| NonZeroU32::new(DEFAULT_RATE_LIMIT_PER_MINUTE).unwrap());

        Self {
            login: RateLimiter::keyed(Quota::per_minute(NonZeroU32::new(5).unwrap())),
            chat_send: RateLimiter::keyed(
                Quota::per_minute(NonZeroU32::new(20).unwrap())
                    .allow_burst(NonZeroU32::new(5).unwrap()),
            ),
            stream_start: RateLimiter::keyed(
                Quota::per_minute(NonZeroU32::new(10).unwrap())
                    .allow_burst(NonZeroU32::new(2).unwrap()),
            ),
            oauth_flow: RateLimiter::keyed(Quota::per_minute(NonZeroU32::new(5).unwrap())),
            confirm_token: RateLimiter::keyed(Quota::per_minute(NonZeroU32::new(5).unwrap())),
            revoke_sessions: RateLimiter::keyed(Quota::per_minute(NonZeroU32::new(3).unwrap())),
            machine_key_rotate: RateLimiter::keyed(Quota::per_minute(NonZeroU32::new(2).unwrap())),
            default_auth: RateLimiter::keyed(Quota::per_minute(default_quota)),
        }
    }
}
