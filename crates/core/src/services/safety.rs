//! Safety coordination service.
//!
//! Owns the panic-disconnect flow. Reachable from every transport
//! identically:
//!
//! * HTTP — `POST /api/v1/safety/panic`
//! * CLI  — `spiritstream-cli safety panic`
//! * Tauri 2 mobile — panic gesture binds to this same call
//!
//! Hotkey registration on desktop is the Tauri shell's responsibility
//! (Tauri 2 global-shortcut plugin); this service handles the actions
//! invoked once a panic fires.
//!
//! # Contract (atomicity)
//!
//! `panic()` is **synchronous-with-timeout client side, fire-and-forget
//! for remote services**. The flow:
//!
//! 1. Stop every active FFmpeg stream (process kill).
//! 2. Disconnect every chat platform (immediate, no graceful close).
//! 3. Disconnect OBS WebSocket.
//! 4. Wipe in-memory secret caches via [`SecretStore::purge_caches`].
//! 5. Record an audit-log entry — audit entries themselves are NOT
//!    wiped; the user needs a forensic trail of when panic fired.
//! 6. Emit `panic_triggered` event so subscribed UIs render the banner.
//!
//! Remote services (Discord webhook for "user panicked" notifications
//! etc.) are intentionally skipped — blocking on platform RTTs would
//! add 5-15s of latency to a panic where every second matters. The
//! audit entry is the durable record.

use std::sync::Arc;
use std::time::Instant;

use crate::errors::CoreError;
use crate::models::{ChatPlatform, Profile};
use crate::services::{
    pii_filter, AuditAction, AuditLogService, ChatManager, FFmpegHandler, ObsWebSocketHandler,
    PiiCheck, PiiMatchMode,
};
use crate::traits::{EventSink, SecretStore};

/// Result returned by a successful panic — surfaced through the event
/// bus and the audit log so the user can see what happened.
#[derive(Debug, Clone)]
pub struct PanicResult {
    pub streams_stopped: usize,
    pub elapsed_ms: u64,
}

/// Coordinates the panic-disconnect flow across services.
pub struct SafetyService {
    ffmpeg: Arc<FFmpegHandler>,
    chat: Arc<ChatManager>,
    obs: Arc<ObsWebSocketHandler>,
    audit: Arc<AuditLogService>,
    events: Arc<dyn EventSink>,
    secrets: Option<Arc<dyn SecretStore>>,
}

impl SafetyService {
    pub fn new(
        ffmpeg: Arc<FFmpegHandler>,
        chat: Arc<ChatManager>,
        obs: Arc<ObsWebSocketHandler>,
        audit: Arc<AuditLogService>,
        events: Arc<dyn EventSink>,
        secrets: Option<Arc<dyn SecretStore>>,
    ) -> Self {
        Self {
            ffmpeg,
            chat,
            obs,
            audit,
            events,
            secrets,
        }
    }

    /// Check an outbound chat message against the active
    /// profile's PII blocklist. Returns `Ok(())` if the message is
    /// clear, or `Err(CoreError::ChatBlockedByPii)` if a phrase
    /// matched. On match, an audit entry is recorded with the stable
    /// `phrase_id` (never the matched text) so a forensic trail exists.
    ///
    /// `platforms` is only used for the audit entry — the filter
    /// decision itself is platform-agnostic.
    pub fn check_outbound_pii(
        &self,
        profile: &Profile,
        platforms: &[ChatPlatform],
        message: &str,
    ) -> Result<(), CoreError> {
        if profile.pii_blocklist.is_empty() {
            return Ok(());
        }
        let mode = if profile.pii_fuzzy {
            PiiMatchMode::Fuzzy
        } else {
            PiiMatchMode::Strict
        };
        if let PiiCheck::Match { phrase_id } =
            pii_filter::check(message, &profile.pii_blocklist, mode)
        {
            // Audit every targeted platform separately. Single-platform
            // chat messages will be the common case (one row); the
            // multi-platform branch records each so a forensic
            // reconstructor can see what would have gone where. If no
            // platforms were specified (defensive — the caller already
            // validated), record one bookkeeping entry under "unknown".
            if platforms.is_empty() {
                let _ = self.audit.record(AuditAction::PiiFilterFired {
                    platform: "unknown".into(),
                    phrase_id: phrase_id.clone(),
                });
            } else {
                for platform in platforms {
                    let _ = self.audit.record(AuditAction::PiiFilterFired {
                        platform: platform.as_str().to_string(),
                        phrase_id: phrase_id.clone(),
                    });
                }
            }
            self.events.emit(
                "pii_filter_fired",
                serde_json::json!({
                    "phraseId": phrase_id,
                    "platformCount": platforms.len(),
                }),
            );
            return Err(CoreError::ChatBlockedByPii { phrase_id });
        }
        Ok(())
    }

    /// Trigger a panic disconnect. Returns a [`PanicResult`] summarising
    /// the action; the result is also emitted as a `panic_triggered`
    /// event and recorded in the audit log.
    ///
    /// Errors from individual steps are logged but never propagated —
    /// a panic call must always finish, even if one of the upstream
    /// services failed. The only failure modes from this method are
    /// audit-log write errors, which we surface so the caller can
    /// decide whether to retry (the upstream actions are already done).
    pub async fn panic(&self) -> Result<PanicResult, CoreError> {
        let started = Instant::now();

        // 1. Stop every active FFmpeg stream. Capture the count before
        //    stop_all clears it so the audit/event can report it.
        let streams_stopped = self.ffmpeg.active_count();
        if let Err(e) = self.ffmpeg.stop_all() {
            log::error!("panic: ffmpeg.stop_all failed: {e}");
        }

        // 2. Disconnect every connected chat platform.
        if let Err(e) = self.chat.disconnect_all().await {
            log::error!("panic: chat.disconnect_all failed: {e}");
        }

        // 3. Disconnect OBS. The handler takes an EventSink generic, so
        //    pass the same one we'll emit through. Errors are logged.
        let obs_event_sink = OneShotEventSink {
            inner: self.events.clone(),
        };
        if let Err(e) = self.obs.disconnect(obs_event_sink).await {
            log::error!("panic: obs.disconnect failed: {e}");
        }

        // 4. Wipe in-memory secret caches. The trait default is a no-op
        //    so an impl that holds no in-memory secrets (file store)
        //    just returns immediately.
        if let Some(secrets) = &self.secrets {
            secrets.purge_caches().await;
        }

        let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

        // 5. Audit. We propagate this error: if the audit write fails
        //    the user has no durable record of the panic, which is
        //    worth surfacing.
        self.audit.record(AuditAction::PanicTriggered {
            streams_stopped,
            elapsed_ms,
        })?;

        // 6. Event. Frontends listen for `panic_triggered` and render
        //    the post-panic banner.
        self.events.emit(
            "panic_triggered",
            serde_json::json!({
                "streamsStopped": streams_stopped,
                "elapsedMs": elapsed_ms,
            }),
        );

        Ok(PanicResult {
            streams_stopped,
            elapsed_ms,
        })
    }
}

/// One-shot adapter that lets [`ObsWebSocketHandler::disconnect`] —
/// which expects an `impl EventSink` by value — accept our `Arc<dyn>`.
struct OneShotEventSink {
    inner: Arc<dyn EventSink>,
}

impl EventSink for OneShotEventSink {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        self.inner.emit(event, payload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::NoopEventSink;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;

    /// Test event sink that counts emissions per event name so we can
    /// assert `panic_triggered` fired exactly once.
    #[derive(Default)]
    struct CountingSink {
        panic_triggers: AtomicUsize,
    }

    impl EventSink for CountingSink {
        fn emit(&self, event: &str, _payload: serde_json::Value) {
            if event == "panic_triggered" {
                self.panic_triggers.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    fn build_service(
        data_dir: &TempDir,
    ) -> (SafetyService, Arc<CountingSink>, Arc<AuditLogService>) {
        let dir = data_dir.path().to_path_buf();
        let ffmpeg = Arc::new(FFmpegHandler::new_with_custom_path(dir.clone(), None));
        let events_for_chat: Arc<dyn EventSink> = Arc::new(NoopEventSink);
        let chat = Arc::new(ChatManager::new(events_for_chat, dir.clone()));
        let obs = Arc::new(ObsWebSocketHandler::new(dir.clone()));
        let audit = Arc::new(AuditLogService::new(dir.clone()).unwrap());
        let counting = Arc::new(CountingSink::default());
        let events: Arc<dyn EventSink> = counting.clone();
        let svc = SafetyService::new(ffmpeg, chat, obs, audit.clone(), events, None);
        (svc, counting, audit)
    }

    #[tokio::test]
    async fn panic_emits_event_and_audit_entry_on_fresh_install() {
        let dir = TempDir::new().unwrap();
        let (svc, sink, audit) = build_service(&dir);
        let result = svc
            .panic()
            .await
            .expect("panic must succeed on fresh install");
        // Nothing was streaming → streams_stopped = 0.
        assert_eq!(result.streams_stopped, 0);
        // Event fired exactly once.
        assert_eq!(sink.panic_triggers.load(Ordering::SeqCst), 1);
        // Audit log has one entry of the right kind.
        let entries = audit.entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(matches!(
            entries[0].action,
            AuditAction::PanicTriggered {
                streams_stopped: 0,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn panic_returns_result_with_elapsed_time() {
        let dir = TempDir::new().unwrap();
        let (svc, _sink, _audit) = build_service(&dir);
        let result = svc.panic().await.unwrap();
        // elapsed_ms is u64; just confirm it deserialised to a finite value.
        // We can't assert a tight upper bound without flakiness on slow CI.
        assert!(
            result.elapsed_ms < 30_000,
            "panic should be quick: {result:?}"
        );
    }

    // --- check_outbound_pii -----------------------------------

    fn fixture_profile(blocklist: Vec<String>, fuzzy: bool) -> Profile {
        use crate::models::{ProfileSettings, RtmpInput};
        Profile {
            id: "p".into(),
            name: "p".into(),
            encrypted: false,
            input: RtmpInput {
                input_type: "rtmp".into(),
                bind_address: "127.0.0.1".into(),
                port: 1935,
                application: "live".into(),
            },
            output_groups: vec![],
            settings: ProfileSettings::default(),
            pii_blocklist: blocklist,
            pii_fuzzy: fuzzy,
            anonymous_logging: true,
            anonymous_salt: crate::services::pseudonymizer::generate_salt(),
        }
    }

    #[tokio::test]
    async fn pii_check_clears_when_blocklist_empty() {
        let dir = TempDir::new().unwrap();
        let (svc, _sink, _audit) = build_service(&dir);
        let profile = fixture_profile(vec![], false);
        let result = svc.check_outbound_pii(&profile, &[ChatPlatform::Twitch], "anything");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn pii_check_blocks_matching_message_and_records_audit() {
        let dir = TempDir::new().unwrap();
        let (svc, _sink, audit) = build_service(&dir);
        let profile = fixture_profile(vec!["realname".into()], false);
        let result =
            svc.check_outbound_pii(&profile, &[ChatPlatform::Twitch], "hi I'm RealName here");
        let err = result.unwrap_err();
        let CoreError::ChatBlockedByPii { phrase_id } = err else {
            panic!("expected ChatBlockedByPii, got {err:?}");
        };
        assert!(!phrase_id.is_empty());
        // Audit must include a PiiFilterFired entry for the targeted platform
        // with the same phrase_id, NOT the matched text.
        let entries = audit.entries().unwrap();
        assert_eq!(entries.len(), 1);
        let AuditAction::PiiFilterFired {
            platform,
            phrase_id: logged_id,
        } = &entries[0].action
        else {
            panic!("expected PiiFilterFired: {:?}", entries[0].action);
        };
        assert_eq!(platform, "twitch");
        assert_eq!(logged_id, &phrase_id);
        let raw = std::fs::read_to_string(audit.log_path()).unwrap();
        assert!(
            !raw.to_lowercase().contains("realname"),
            "audit leaked matched text: {raw}"
        );
    }

    #[tokio::test]
    async fn pii_check_audits_each_targeted_platform_on_match() {
        let dir = TempDir::new().unwrap();
        let (svc, _sink, audit) = build_service(&dir);
        let profile = fixture_profile(vec!["alex".into()], false);
        let _ = svc.check_outbound_pii(
            &profile,
            &[ChatPlatform::Twitch, ChatPlatform::YouTube],
            "hi alex",
        );
        let entries = audit.entries().unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn pii_check_fuzzy_catches_leet_when_enabled() {
        let dir = TempDir::new().unwrap();
        let (svc, _sink, _audit) = build_service(&dir);
        let profile = fixture_profile(vec!["alex".into()], true);
        let result = svc.check_outbound_pii(&profile, &[ChatPlatform::Twitch], "hi @l3x");
        assert!(matches!(result, Err(CoreError::ChatBlockedByPii { .. })));
    }

    #[tokio::test]
    async fn pii_check_strict_misses_leet_by_default() {
        let dir = TempDir::new().unwrap();
        let (svc, _sink, _audit) = build_service(&dir);
        let profile = fixture_profile(vec!["alex".into()], false);
        let result = svc.check_outbound_pii(&profile, &[ChatPlatform::Twitch], "hi @l3x");
        assert!(result.is_ok(), "strict mode must not match leet variants");
    }

    #[tokio::test]
    async fn panic_is_idempotent_under_repeated_calls() {
        let dir = TempDir::new().unwrap();
        let (svc, sink, audit) = build_service(&dir);
        svc.panic().await.unwrap();
        svc.panic().await.unwrap();
        svc.panic().await.unwrap();
        assert_eq!(sink.panic_triggers.load(Ordering::SeqCst), 3);
        // Each panic produces a distinct audit entry.
        assert_eq!(audit.entries().unwrap().len(), 3);
    }
}
