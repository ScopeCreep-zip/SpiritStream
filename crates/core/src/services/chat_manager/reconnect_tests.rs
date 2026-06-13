//! `reconnect` vs `connect` regression tests.
//!
//! A read-only Twitch session (anonymous fallback) reports
//! `is_connected() == true`, so a plain `connect` rejects with
//! `already_connected` and can't swap in a freshly-refreshed token. The
//! recovery path (manual retry + re-auth force) uses `reconnect`, which must
//! bypass that guard and cleanly disconnect-then-reconnect the live session.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tempfile::TempDir;

use crate::errors::CoreError;
use crate::models::{
    ChatConfig, ChatConnectionStatus, ChatCredentials, ChatMessage, ChatPlatform,
};
use crate::services::chat::{ChatPlatform as ChatPlatformTrait, PlatformResult};
use crate::services::events::NoopEventSink;
use crate::services::EventSink;

use super::ChatManager;

/// A connector that always reports Connected + read-only and counts its
/// connect/disconnect calls.
struct LiveReadOnlyConnector {
    connect_calls: Arc<AtomicUsize>,
    disconnect_calls: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl ChatPlatformTrait for LiveReadOnlyConnector {
    async fn connect(
        &mut self,
        _credentials: crate::models::ChatCredentials,
        _message_tx: tokio::sync::mpsc::Sender<ChatMessage>,
    ) -> PlatformResult<()> {
        self.connect_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    async fn disconnect(&mut self) -> PlatformResult<()> {
        self.disconnect_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn status(&self) -> ChatConnectionStatus {
        ChatConnectionStatus::Connected
    }
    fn message_count(&self) -> u64 {
        0
    }
    fn platform_name(&self) -> &'static str {
        "twitch"
    }
    fn can_send(&self) -> bool {
        false
    }
}

fn manager() -> Arc<ChatManager> {
    let dir = TempDir::new().unwrap();
    let data_dir = dir.path().to_path_buf();
    std::mem::forget(dir); // writer task keeps the dir; leak for test lifetime
    let event_sink: Arc<dyn EventSink> = Arc::new(NoopEventSink);
    Arc::new(ChatManager::new(event_sink, data_dir.clone(), data_dir))
}

fn twitch_config() -> ChatConfig {
    ChatConfig {
        platform: ChatPlatform::Twitch,
        enabled: true,
        credentials: ChatCredentials::Twitch {
            channel: "mychan".into(),
            auth: None,
        },
    }
}

#[tokio::test]
async fn connect_rejects_already_connected_but_reconnect_swaps_the_session() {
    let mgr = manager();
    let connect_calls = Arc::new(AtomicUsize::new(0));
    let disconnect_calls = Arc::new(AtomicUsize::new(0));
    mgr.insert_test_connector(
        ChatPlatform::Twitch,
        Box::new(LiveReadOnlyConnector {
            connect_calls: connect_calls.clone(),
            disconnect_calls: disconnect_calls.clone(),
        }),
    )
    .await;

    // Plain connect must NOT touch a live session.
    let err = mgr.connect(twitch_config()).await.unwrap_err();
    match err {
        CoreError::ValidationFailed { reasons } => assert!(
            reasons
                .iter()
                .any(|r| r.code == "chat_platform_already_connected"),
            "connect should reject an already-connected platform"
        ),
        other => panic!("expected ValidationFailed, got {other:?}"),
    }
    assert_eq!(connect_calls.load(Ordering::SeqCst), 0);
    assert_eq!(disconnect_calls.load(Ordering::SeqCst), 0);

    // reconnect bypasses the guard: close the old session, then connect.
    mgr.reconnect(twitch_config()).await.unwrap();
    assert_eq!(
        disconnect_calls.load(Ordering::SeqCst),
        1,
        "reconnect closes the read-only session first"
    );
    assert_eq!(
        connect_calls.load(Ordering::SeqCst),
        1,
        "reconnect re-establishes with the (fresh) credentials"
    );
}

#[tokio::test]
async fn reconnect_clears_disconnect_intent() {
    let mgr = manager();
    // Pretend the user had deliberately disconnected.
    mgr.insert_test_connector(
        ChatPlatform::Twitch,
        Box::new(LiveReadOnlyConnector {
            connect_calls: Arc::new(AtomicUsize::new(0)),
            disconnect_calls: Arc::new(AtomicUsize::new(0)),
        }),
    )
    .await;
    mgr.disconnect(ChatPlatform::Twitch).await.unwrap();
    assert!(mgr.is_disconnect_intended(ChatPlatform::Twitch).await);

    // Re-insert a live connector and reconnect; intent must clear so the
    // reconnect/auto-connect loops keep it alive.
    mgr.insert_test_connector(
        ChatPlatform::Twitch,
        Box::new(LiveReadOnlyConnector {
            connect_calls: Arc::new(AtomicUsize::new(0)),
            disconnect_calls: Arc::new(AtomicUsize::new(0)),
        }),
    )
    .await;
    mgr.reconnect(twitch_config()).await.unwrap();
    assert!(!mgr.is_disconnect_intended(ChatPlatform::Twitch).await);
}
