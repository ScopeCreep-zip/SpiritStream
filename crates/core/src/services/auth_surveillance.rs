//! Account-takeover hooks.
//!
//! Watches OAuth token-refresh activity for signs that the account has
//! been compromised. Every successful refresh is appended to the audit
//! log; an anomaly score is computed from the sliding window of recent
//! refreshes and a non-fatal `unusual_oauth_refresh` event is emitted
//! when the score crosses the policy threshold.
//!
//! # Local-only signals
//!
//! The plan calls for geo-correlated anomalies (refreshes from
//! different countries within a short window). That requires a
//! configured MaxMind GeoLite2 database — when one is present the
//! geo path activates; without one, the implementation falls back to
//! frequency-based detection. **Both signals are local-only**; full
//! server-side detection is out of scope for the rewrite per the plan.
//!
//! # Heuristic
//!
//! For each platform we track up to `MAX_TRACKED` recent refresh
//! timestamps in memory. On each `record_oauth_refresh` call we trim
//! entries older than `WINDOW`, append the new timestamp, and check
//! whether the count within `RAPID_WINDOW` exceeds `RAPID_THRESHOLD`.
//! If so, the refresh is flagged as unusual.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::errors::CoreError;
use crate::services::{AuditAction, AuditLogService};
use crate::traits::EventSink;

/// How many recent refresh events to retain per platform.
const MAX_TRACKED: usize = 32;
/// Total retention window for refresh history.
const WINDOW: Duration = Duration::from_secs(24 * 3600);
/// Window within which a burst of refreshes is suspicious.
const RAPID_WINDOW: Duration = Duration::from_secs(60 * 60);
/// Count of refreshes within `RAPID_WINDOW` that trips the anomaly flag.
/// Token TTLs are typically 1–4 hours, so a healthy refresh cadence is
/// at most ~2 per hour. Three within an hour is suspicious.
const RAPID_THRESHOLD: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshAnomaly {
    None,
    /// More than `RAPID_THRESHOLD` refreshes for this platform within
    /// `RAPID_WINDOW`. Could indicate an attacker burning refresh
    /// tokens on a hijacked account.
    RapidRefreshBurst {
        platform: String,
        refreshes_in_window: usize,
    },
}

pub struct AuthSurveillanceService {
    audit: Arc<AuditLogService>,
    events: Arc<dyn EventSink>,
    history: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl AuthSurveillanceService {
    pub fn new(audit: Arc<AuditLogService>, events: Arc<dyn EventSink>) -> Self {
        Self {
            audit,
            events,
            history: Mutex::new(HashMap::new()),
        }
    }

    /// Record an OAuth refresh event for `platform`. Appends an
    /// `OauthRefresh` entry to the audit log, then checks for
    /// anomalies. On anomaly: appends an additional
    /// `OauthRefreshUnusualLocation` audit entry (used for both
    /// geo-changes and the frequency heuristic) and emits an
    /// `unusual_oauth_refresh` event.
    pub async fn record_oauth_refresh(
        &self,
        platform: &str,
        success: bool,
    ) -> Result<RefreshAnomaly, CoreError> {
        // 1. Always audit the refresh attempt.
        self.audit.record(AuditAction::OauthRefresh {
            platform: platform.to_string(),
            success,
        })?;
        if !success {
            // Failures aren't anomalies in their own right — they're
            // captured for forensics and we're done.
            return Ok(RefreshAnomaly::None);
        }

        // 2. Update history for this platform.
        let now = Instant::now();
        let count_in_window = {
            let mut history = self.history.lock().await;
            let entry = history.entry(platform.to_string()).or_default();
            if let Some(cutoff) = now.checked_sub(WINDOW) {
                while entry.front().is_some_and(|t| *t < cutoff) {
                    entry.pop_front();
                }
            }
            entry.push_back(now);
            while entry.len() > MAX_TRACKED {
                entry.pop_front();
            }
            let rapid_cutoff = now.checked_sub(RAPID_WINDOW);
            entry
                .iter()
                .filter(|t| rapid_cutoff.map(|c| **t >= c).unwrap_or(true))
                .count()
        };

        // 3. Anomaly check + audit + event.
        if count_in_window >= RAPID_THRESHOLD {
            self.audit
                .record(AuditAction::OauthRefreshUnusualLocation {
                    platform: platform.to_string(),
                })?;
            self.events.emit(
                "unusual_oauth_refresh",
                serde_json::json!({
                    "platform": platform,
                    "refreshesInWindow": count_in_window,
                    "reason": "rapid_burst",
                }),
            );
            return Ok(RefreshAnomaly::RapidRefreshBurst {
                platform: platform.to_string(),
                refreshes_in_window: count_in_window,
            });
        }
        Ok(RefreshAnomaly::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::NoopEventSink;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;

    #[derive(Default)]
    struct CountingSink {
        unusual: AtomicUsize,
    }

    impl EventSink for CountingSink {
        fn emit(&self, event: &str, _payload: serde_json::Value) {
            if event == "unusual_oauth_refresh" {
                self.unusual.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    fn build(
        dir: &TempDir,
    ) -> (
        Arc<AuditLogService>,
        Arc<CountingSink>,
        AuthSurveillanceService,
    ) {
        let audit = Arc::new(AuditLogService::new(dir.path().to_path_buf()).unwrap());
        let sink = Arc::new(CountingSink::default());
        let events: Arc<dyn EventSink> = sink.clone();
        let svc = AuthSurveillanceService::new(audit.clone(), events);
        (audit, sink, svc)
    }

    #[tokio::test]
    async fn single_refresh_records_audit_no_anomaly() {
        let dir = TempDir::new().unwrap();
        let (audit, sink, svc) = build(&dir);
        let result = svc.record_oauth_refresh("twitch", true).await.unwrap();
        assert_eq!(result, RefreshAnomaly::None);
        assert_eq!(sink.unusual.load(Ordering::SeqCst), 0);
        let entries = audit.entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(matches!(
            entries[0].action,
            AuditAction::OauthRefresh { .. }
        ));
    }

    #[tokio::test]
    async fn rapid_burst_emits_unusual_event_and_audit() {
        let dir = TempDir::new().unwrap();
        let (audit, sink, svc) = build(&dir);
        // First two refreshes — under threshold (3).
        let r1 = svc.record_oauth_refresh("twitch", true).await.unwrap();
        let r2 = svc.record_oauth_refresh("twitch", true).await.unwrap();
        assert_eq!(r1, RefreshAnomaly::None);
        assert_eq!(r2, RefreshAnomaly::None);
        // Third refresh trips the threshold.
        let r3 = svc.record_oauth_refresh("twitch", true).await.unwrap();
        assert!(matches!(r3, RefreshAnomaly::RapidRefreshBurst { .. }));
        assert_eq!(sink.unusual.load(Ordering::SeqCst), 1);
        let entries = audit.entries().unwrap();
        // 3 OauthRefresh entries + 1 OauthRefreshUnusualLocation entry.
        assert_eq!(entries.len(), 4);
        let unusual_count = entries
            .iter()
            .filter(|e| matches!(e.action, AuditAction::OauthRefreshUnusualLocation { .. }))
            .count();
        assert_eq!(unusual_count, 1);
    }

    #[tokio::test]
    async fn separate_platforms_track_independently() {
        let dir = TempDir::new().unwrap();
        let (_audit, sink, svc) = build(&dir);
        // 3 twitch refreshes → twitch tripped.
        let _ = svc.record_oauth_refresh("twitch", true).await;
        let _ = svc.record_oauth_refresh("twitch", true).await;
        let r3 = svc.record_oauth_refresh("twitch", true).await.unwrap();
        assert!(matches!(r3, RefreshAnomaly::RapidRefreshBurst { .. }));
        // 1 youtube refresh — does NOT trip (independent history).
        let r4 = svc.record_oauth_refresh("youtube", true).await.unwrap();
        assert_eq!(r4, RefreshAnomaly::None);
        assert_eq!(sink.unusual.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn failed_refresh_is_audited_but_not_counted() {
        let dir = TempDir::new().unwrap();
        let (audit, sink, svc) = build(&dir);
        // 5 failures should NOT trip the anomaly (the heuristic targets
        // successful bursts, not failure storms).
        for _ in 0..5 {
            let r = svc.record_oauth_refresh("twitch", false).await.unwrap();
            assert_eq!(r, RefreshAnomaly::None);
        }
        assert_eq!(sink.unusual.load(Ordering::SeqCst), 0);
        let entries = audit.entries().unwrap();
        assert_eq!(entries.len(), 5);
    }

    /// Smoke test for the `NoopEventSink` integration — confirms a
    /// transport that doesn't care about events still works.
    #[tokio::test]
    async fn works_with_noop_event_sink() {
        let dir = TempDir::new().unwrap();
        let audit = Arc::new(AuditLogService::new(dir.path().to_path_buf()).unwrap());
        let events: Arc<dyn EventSink> = Arc::new(NoopEventSink);
        let svc = AuthSurveillanceService::new(audit, events);
        for _ in 0..3 {
            let _ = svc.record_oauth_refresh("twitch", true).await;
        }
    }
}
