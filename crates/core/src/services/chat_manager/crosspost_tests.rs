//! Crosspost safety regression tests.
//!
//! Pre-fix, the crosspost path re-broadcast inbound third-party text
//! from the streamer's own accounts with NO outbound PII gate and no
//! length check — a harasser posting the streamer's deadname in one
//! chat got it relayed to every other platform under the streamer's
//! name. These tests pin the gate: blocked when a phrase matches,
//! refused entirely when no guard is wired, delivered when clean.

use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use async_trait::async_trait;
use tempfile::TempDir;
use tokio::sync::mpsc;

use crate::models::{ChatConnectionStatus, ChatCredentials, ChatMessage, ChatPlatform};
use crate::services::chat::{BoxedPlatform, ChatPlatform as ChatPlatformTrait, PlatformResult};
use crate::services::events::NoopEventSink;
use crate::services::{
    AuditAction, AuditLogService, EncryptedFileSecretStore, EventSink, FFmpegHandler,
    ObsWebSocketHandler, SafetyService,
};
use crate::traits::SecretStore;

use super::ChatManager;

/// Connector that records every message it is asked to send.
struct RecordingConnector {
    name: &'static str,
    sent: Arc<StdMutex<Vec<String>>>,
}

#[async_trait]
impl ChatPlatformTrait for RecordingConnector {
    async fn connect(
        &mut self,
        _credentials: ChatCredentials,
        _message_tx: mpsc::Sender<ChatMessage>,
    ) -> PlatformResult<()> {
        Ok(())
    }
    async fn disconnect(&mut self) -> PlatformResult<()> {
        Ok(())
    }
    fn status(&self) -> ChatConnectionStatus {
        ChatConnectionStatus::Connected
    }
    fn message_count(&self) -> u64 {
        0
    }
    fn platform_name(&self) -> &'static str {
        self.name
    }
    async fn send_message(&mut self, message: String) -> PlatformResult<()> {
        self.sent.lock().unwrap().push(message);
        Ok(())
    }
    fn can_send(&self) -> bool {
        true
    }
}

struct Fixture {
    _dir: TempDir,
    mgr: Arc<ChatManager>,
    safety: Arc<SafetyService>,
    audit: Arc<AuditLogService>,
    twitch_sent: Arc<StdMutex<Vec<String>>>,
}

async fn fixture() -> Fixture {
    let dir = TempDir::new().unwrap();
    let data_dir = dir.path().to_path_buf();
    let event_sink: Arc<dyn EventSink> = Arc::new(NoopEventSink);
    let mgr = Arc::new(ChatManager::new(event_sink.clone(), data_dir.clone()));
    let audit = Arc::new(AuditLogService::new_for_tests(data_dir.clone()).unwrap());
    mgr.set_audit_log(audit.clone());
    let ffmpeg = Arc::new(
        FFmpegHandler::new_with_custom_path(data_dir.clone(), None).expect("test fixture"),
    );
    let obs = Arc::new(ObsWebSocketHandler::new(data_dir.clone()));
    let secrets: Arc<dyn SecretStore> = Arc::new(EncryptedFileSecretStore::new(data_dir.clone()));
    let phrase_id_key = crate::services::Encryption::derive_machine_subkey(
        &data_dir,
        crate::services::PHRASE_ID_KEY_INFO,
    )
    .expect("derive phrase-id key");
    let safety = Arc::new(SafetyService::new(
        ffmpeg,
        mgr.clone(),
        obs,
        audit.clone(),
        event_sink,
        secrets,
        phrase_id_key,
    ));

    // Crosspost setup: messages arrive "from" YouTube, fan out to Twitch.
    let twitch_sent: Arc<StdMutex<Vec<String>>> = Arc::new(StdMutex::new(Vec::new()));
    let twitch: BoxedPlatform = Box::new(RecordingConnector {
        name: "twitch",
        sent: twitch_sent.clone(),
    });
    mgr.insert_test_connector(ChatPlatform::Twitch, twitch).await;
    mgr.set_crosspost_enabled(true);
    mgr.set_send_enabled(ChatPlatform::Twitch, true).await;

    Fixture {
        _dir: dir,
        mgr,
        safety,
        audit,
        twitch_sent,
    }
}

async fn push_inbound(mgr: &ChatManager, text: &str) {
    let msg = ChatMessage::new(ChatPlatform::YouTube, "viewer".into(), text.into());
    mgr.message_tx.send(msg).await.expect("message handler alive");
}

/// Wait for the async message handler + crosspost spawn to settle.
async fn settle() {
    tokio::time::sleep(Duration::from_millis(250)).await;
}

#[tokio::test]
async fn crosspost_delivers_clean_message_when_guard_is_wired() {
    let f = fixture().await;
    f.mgr.set_outbound_guard(f.safety.clone());
    f.mgr.set_pii_policy(vec!["deadname".into()], false);

    push_inbound(&f.mgr, "totally normal message").await;
    settle().await;

    assert_eq!(
        f.twitch_sent.lock().unwrap().as_slice(),
        ["totally normal message"]
    );
}

#[tokio::test]
async fn crosspost_blocks_message_matching_pii_blocklist() {
    let f = fixture().await;
    f.mgr.set_outbound_guard(f.safety.clone());
    f.mgr.set_pii_policy(vec!["deadname".into()], false);

    push_inbound(&f.mgr, "hey everyone her name is DeadName lol").await;
    settle().await;

    assert!(
        f.twitch_sent.lock().unwrap().is_empty(),
        "blocked message must never be rebroadcast"
    );
    // The guard recorded the block in the audit chain (phrase_id only).
    let entries = f.audit.entries().unwrap();
    assert!(
        entries
            .iter()
            .any(|e| matches!(e.action, AuditAction::ChatMessagePiiBlocked { .. })),
        "expected ChatMessagePiiBlocked audit entry"
    );
    let raw = std::fs::read_to_string(f.audit.log_path()).unwrap();
    assert!(
        !raw.to_lowercase().contains("deadname"),
        "audit leaked the matched text"
    );
}

#[tokio::test]
async fn crosspost_refuses_entirely_when_no_guard_is_wired() {
    let f = fixture().await;
    // No set_outbound_guard — simulates a registry wiring bug. Even a
    // clean message must NOT be rebroadcast unchecked (fail loud).
    push_inbound(&f.mgr, "totally normal message").await;
    settle().await;

    assert!(
        f.twitch_sent.lock().unwrap().is_empty(),
        "crosspost must refuse to send while the PII guard is unwired"
    );
}

/// Connector whose disconnect never resolves — simulates a wedged
/// network close during a panic teardown.
struct HangingConnector;

#[async_trait]
impl ChatPlatformTrait for HangingConnector {
    async fn connect(
        &mut self,
        _credentials: ChatCredentials,
        _message_tx: mpsc::Sender<ChatMessage>,
    ) -> PlatformResult<()> {
        Ok(())
    }
    async fn disconnect(&mut self) -> PlatformResult<()> {
        std::future::pending::<()>().await;
        Ok(())
    }
    fn status(&self) -> ChatConnectionStatus {
        ChatConnectionStatus::Connected
    }
    fn message_count(&self) -> u64 {
        0
    }
    fn platform_name(&self) -> &'static str {
        "hanging"
    }
    async fn send_message(&mut self, _message: String) -> PlatformResult<()> {
        Ok(())
    }
    fn can_send(&self) -> bool {
        false
    }
}

/// Panic-path liveness: one wedged connector must not stall the whole
/// teardown — `disconnect_all` times it out (5s) and reports the error.
/// Paused clock makes the timeout instantaneous in test time.
#[tokio::test(start_paused = true)]
async fn disconnect_all_times_out_hung_connectors_instead_of_stalling() {
    let f = fixture().await;
    f.mgr
        .insert_test_connector(ChatPlatform::Kick, Box::new(HangingConnector))
        .await;

    let result = f.mgr.disconnect_all("panic_triggered").await;
    let err = result.expect_err("hung connector must surface as an error");
    assert!(
        format!("{err:?}").contains("timed out"),
        "expected timeout error, got: {err:?}"
    );
}

#[tokio::test]
async fn crosspost_skips_platforms_whose_length_limit_is_exceeded() {
    let f = fixture().await;
    f.mgr.set_outbound_guard(f.safety.clone());
    f.mgr.set_pii_policy(Vec::new(), false);

    let limit = ChatPlatform::Twitch.max_message_chars();
    let too_long = "x".repeat(limit + 10);
    push_inbound(&f.mgr, &too_long).await;
    settle().await;

    assert!(
        f.twitch_sent.lock().unwrap().is_empty(),
        "over-limit crosspost must be skipped, mirroring the send path"
    );
}
