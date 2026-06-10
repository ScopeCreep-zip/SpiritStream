use std::sync::Arc;

use async_trait::async_trait;
use tempfile::TempDir;
use tokio::sync::mpsc;

use crate::errors::CoreError;
use crate::models::{ChatConnectionStatus, ChatCredentials, ChatMessage, ChatPlatform};
use crate::services::chat::{BoxedPlatform, ChatPlatform as ChatPlatformTrait, PlatformResult};
use crate::services::events::NoopEventSink;
use crate::services::{
    AuditAction, AuditLogService, EncryptedFileSecretStore, EventSink, FFmpegHandler,
    ObsWebSocketHandler, SafetyService,
};
use crate::traits::SecretStore;

use super::ChatManager;

/// Always-ready test connector. `send_message` returns `Ok(())` so
/// `ChatManager::send_message` reaches the audit-record branch on
/// the success path — that branch is otherwise unexercised by unit
/// tests because the real connectors require network.
struct AlwaysOkConnector {
    name: &'static str,
}

#[async_trait]
impl ChatPlatformTrait for AlwaysOkConnector {
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
    async fn send_message(&mut self, _message: String) -> PlatformResult<()> {
        Ok(())
    }
    fn can_send(&self) -> bool {
        true
    }
}

impl ChatManager {
    /// Test-only: inject a pre-built connector into the platforms
    /// map. Bypasses `connect()` so unit tests can exercise the
    /// post-connection code paths (send, disconnect, audit) without
    /// network. Production code never reaches this — the registry's
    /// `BoxedPlatform` factory is the only path that builds
    /// connectors at runtime.
    async fn insert_test_connector(&self, platform: ChatPlatform, connector: BoxedPlatform) {
        self.platforms.lock().await.insert(platform, connector);
    }
}

/// Build a (ChatManager, SafetyService, AuditLogService) triple
/// sharing the same audit instance, so a test can both call
/// `send_message(..., &safety)` and inspect the audit chain.
fn fixture() -> (
    TempDir,
    Arc<ChatManager>,
    SafetyService,
    Arc<AuditLogService>,
) {
    let dir = TempDir::new().unwrap();
    let data_dir = dir.path().to_path_buf();
    let event_sink: Arc<dyn EventSink> = Arc::new(NoopEventSink);
    let mgr = Arc::new(ChatManager::new(event_sink.clone(), data_dir.clone()));
    let audit = Arc::new(AuditLogService::new(data_dir.clone()).unwrap());
    mgr.set_audit_log(audit.clone());
    let ffmpeg = Arc::new(
        FFmpegHandler::new_with_custom_path(data_dir.clone(), None).expect("test fixture"),
    );
    let obs = Arc::new(ObsWebSocketHandler::new(data_dir.clone()));
    let secrets: Arc<dyn SecretStore> = Arc::new(EncryptedFileSecretStore::new(data_dir.clone()));
    let safety = SafetyService::new(ffmpeg, mgr.clone(), obs, audit.clone(), event_sink, secrets);
    (dir, mgr, safety, audit)
}

/// Plan-cited variant: messages over the per-platform char limit must
/// fail with `ChatMessageLengthExceeded` BEFORE the wire / PII filter.
#[tokio::test]
async fn over_length_message_returns_chat_message_length_exceeded() {
    let (_dir, mgr, safety, _audit) = fixture();
    let limit = ChatPlatform::Twitch.max_message_chars();
    let too_long = "x".repeat(limit + 50);
    let results = mgr
        .send_message(too_long, &[ChatPlatform::Twitch], None, &safety)
        .await;
    assert_eq!(results.len(), 1);
    match &results[0] {
        (
            ChatPlatform::Twitch,
            Err(CoreError::ChatMessageLengthExceeded {
                platform,
                limit: l,
                actual,
            }),
        ) => {
            assert_eq!(platform, "twitch");
            assert_eq!(*l, limit);
            assert_eq!(*actual, limit + 50);
        }
        other => panic!("expected ChatMessageLengthExceeded, got {other:?}"),
    }
}

/// Plan-cited variant: sending to a platform that has no connector
/// initialized must fail with `ChatPlatformNotConnected`. A freshly-
/// built manager has no platforms attached.
#[tokio::test]
async fn unconnected_platform_returns_chat_platform_not_connected() {
    let (_dir, mgr, safety, _audit) = fixture();
    let results = mgr
        .send_message("hello".to_string(), &[ChatPlatform::Twitch], None, &safety)
        .await;
    assert_eq!(results.len(), 1);
    match &results[0] {
        (ChatPlatform::Twitch, Err(CoreError::ChatPlatformNotConnected { platform })) => {
            assert_eq!(platform, "twitch");
        }
        other => panic!("expected ChatPlatformNotConnected, got {other:?}"),
    }
}

/// Multiple platforms must each surface their own typed failure — the
/// per-platform Result shape is the API contract; ChatSendResult on the
/// wire carries the kind via `errorCode`.
#[tokio::test]
async fn per_platform_results_are_independent() {
    let (_dir, mgr, safety, _audit) = fixture();
    let results = mgr
        .send_message(
            "ok".to_string(),
            &[ChatPlatform::Twitch, ChatPlatform::YouTube],
            None,
            &safety,
        )
        .await;
    assert_eq!(results.len(), 2);
    for (_, r) in &results {
        assert!(matches!(r, Err(CoreError::ChatPlatformNotConnected { .. })));
    }
}

/// Plan-cited Phase A verification: on a send that reaches at least
/// one platform successfully, the audit chain must contain
/// `ChatMessageSent` with the successful platform list and char
/// count. Uses an injected always-ok connector to avoid network.
#[tokio::test]
async fn successful_send_records_chat_message_sent_audit() {
    let (_dir, mgr, safety, audit) = fixture();
    let connector: BoxedPlatform = Box::new(AlwaysOkConnector { name: "twitch" });
    mgr.insert_test_connector(ChatPlatform::Twitch, connector)
        .await;

    let results = mgr
        .send_message(
            "hello world".to_string(),
            &[ChatPlatform::Twitch],
            None,
            &safety,
        )
        .await;
    assert_eq!(results.len(), 1);
    assert!(matches!(results[0], (ChatPlatform::Twitch, Ok(()))));

    let entries = audit.entries().unwrap();
    let sent: Vec<_> = entries
        .iter()
        .filter_map(|e| match &e.action {
            AuditAction::ChatMessageSent {
                platforms,
                char_count,
            } => Some((platforms.clone(), *char_count)),
            _ => None,
        })
        .collect();
    assert_eq!(sent.len(), 1, "expected exactly one ChatMessageSent");
    assert_eq!(sent[0].0, vec!["twitch".to_string()]);
    assert_eq!(sent[0].1, "hello world".chars().count());
}

/// PII gate is atomic: a message matching the blocklist must fail
/// EVERY targeted platform with `ChatBlockedByPii` — never per-platform
/// partial results — and must not record `ChatMessageSent`.
#[tokio::test]
async fn pii_match_blocks_all_targets_atomically() {
    let (_dir, mgr, safety, audit) = fixture();
    let policy = Some((vec!["alice".into()], false));
    let results = mgr
        .send_message(
            "hi alice".to_string(),
            &[ChatPlatform::Twitch, ChatPlatform::YouTube],
            policy,
            &safety,
        )
        .await;
    assert_eq!(results.len(), 2);
    for (_, r) in &results {
        assert!(matches!(r, Err(CoreError::ChatBlockedByPii { .. })));
    }
    // Audit must contain exactly one aggregate ChatMessagePiiBlocked
    // entry and *not* a ChatMessageSent entry.
    let entries = audit.entries().unwrap();
    assert!(entries
        .iter()
        .all(|e| !matches!(e.action, AuditAction::ChatMessageSent { .. })));
    let blocked: Vec<_> = entries
        .iter()
        .filter(|e| matches!(e.action, AuditAction::ChatMessagePiiBlocked { .. }))
        .collect();
    assert_eq!(blocked.len(), 1);
}
