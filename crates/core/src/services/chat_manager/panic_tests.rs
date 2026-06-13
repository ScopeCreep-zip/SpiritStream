//! Panic ("make it disappear") regression tests.
//!
//! Panic must not just clear what's already stored — it must also drop
//! any message that was *in flight* when the button was pressed (sitting
//! in the handler channel or mid-pseudonymize). Otherwise a straggler
//! lands in the freshly-recreated history file, the recent ring, or the
//! UI event stream *after* the wipe, defeating the safety contract for
//! this population. The fix is a `purge_epoch_ms` boundary stamped at
//! panic time; the message handler drops anything created at/before it.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tempfile::TempDir;

use crate::models::{ChatConnectionStatus, ChatCredentials, ChatMessage, ChatPlatform};
use crate::services::chat::{BoxedPlatform, ChatPlatform as ChatPlatformTrait, PlatformResult};
use crate::services::events::NoopEventSink;
use crate::services::EventSink;

use super::ChatManager;

/// Records how many `chat_message` events were emitted to the frontend, so
/// a test can assert a gated straggler never reaches the UI.
struct CountingEventSink {
    chat_message_emits: Arc<AtomicUsize>,
}

impl EventSink for CountingEventSink {
    fn emit(&self, event: &str, _payload: Value) {
        if event == "chat_message" {
            self.chat_message_emits.fetch_add(1, Ordering::SeqCst);
        }
    }
}

struct ConnectedConnector;

#[async_trait::async_trait]
impl ChatPlatformTrait for ConnectedConnector {
    async fn connect(
        &mut self,
        _credentials: ChatCredentials,
        _message_tx: tokio::sync::mpsc::Sender<ChatMessage>,
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
        "twitch"
    }
    async fn send_message(&mut self, _message: String) -> PlatformResult<()> {
        Ok(())
    }
    fn can_send(&self) -> bool {
        true
    }
}

/// Connector that emits a straggler message *during* its `disconnect()` —
/// i.e. inside the panic teardown window — then lingers briefly so the
/// handler has time to process it. Models a chat socket that delivers one
/// last buffered line while being torn down.
struct StragglerOnDisconnect {
    tx: tokio::sync::mpsc::Sender<ChatMessage>,
    straggler_ts: i64,
}

#[async_trait::async_trait]
impl ChatPlatformTrait for StragglerOnDisconnect {
    async fn connect(
        &mut self,
        _credentials: ChatCredentials,
        _message_tx: tokio::sync::mpsc::Sender<ChatMessage>,
    ) -> PlatformResult<()> {
        Ok(())
    }
    async fn disconnect(&mut self) -> PlatformResult<()> {
        let mut msg =
            ChatMessage::new(ChatPlatform::Twitch, "viewer".into(), "teardown-straggler".into());
        msg.timestamp = self.straggler_ts;
        let _ = self.tx.send(msg).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
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
    async fn send_message(&mut self, _message: String) -> PlatformResult<()> {
        Ok(())
    }
    fn can_send(&self) -> bool {
        true
    }
}

fn manager() -> Arc<ChatManager> {
    manager_with_sink(Arc::new(NoopEventSink))
}

fn manager_with_sink(event_sink: Arc<dyn EventSink>) -> Arc<ChatManager> {
    let dir = TempDir::new().unwrap();
    let data_dir = dir.path().to_path_buf();
    // Leak the TempDir for the test's lifetime — the writer task keeps the
    // dir alive; dropping it mid-test would race the background task.
    std::mem::forget(dir);
    Arc::new(ChatManager::new(event_sink, data_dir.clone(), data_dir))
}

async fn push(mgr: &ChatManager, text: &str, timestamp: i64) {
    let mut msg = ChatMessage::new(ChatPlatform::Twitch, "viewer".into(), text.into());
    msg.timestamp = timestamp;
    mgr.message_tx.send(msg).await.expect("handler alive");
}

async fn settle() {
    tokio::time::sleep(Duration::from_millis(150)).await;
}

#[tokio::test]
async fn purge_boundary_drops_messages_at_or_before_it() {
    let mgr = manager();
    let boundary = 1_000_000_000_000_i64;
    mgr.purge_epoch_ms.store(boundary, Ordering::SeqCst);

    // A straggler created before (and exactly at) the panic boundary must
    // never reach the ring.
    push(&mgr, "straggler-before", boundary - 1).await;
    push(&mgr, "straggler-exact", boundary).await;
    // A genuinely new message created after the boundary passes through.
    push(&mgr, "live-after", boundary + 1).await;
    settle().await;

    let ring = mgr.recent_messages().await;
    let texts: Vec<&str> = ring.iter().map(|m| m.message.as_str()).collect();
    assert_eq!(
        texts,
        ["live-after"],
        "only the post-boundary message survives the purge gate"
    );
}

#[tokio::test]
async fn panic_disconnect_all_sets_boundary_and_clears_ring() {
    let mgr = manager();
    let connector: BoxedPlatform = Box::new(ConnectedConnector);
    mgr.insert_test_connector(ChatPlatform::Twitch, connector)
        .await;

    // A normal message lands in the ring before the panic.
    let before_panic = chrono::Utc::now().timestamp_millis();
    push(&mgr, "pre-panic", before_panic).await;
    settle().await;
    assert_eq!(mgr.recent_messages().await.len(), 1);

    mgr.disconnect_all("panic_triggered").await.unwrap();

    // Ring wiped and the purge boundary stamped at/after the pre-panic msg.
    assert!(
        mgr.recent_messages().await.is_empty(),
        "panic must clear the recent ring"
    );
    let boundary = mgr.purge_epoch_ms.load(Ordering::SeqCst);
    assert!(boundary >= before_panic, "panic must stamp a purge boundary");

    // A straggler that was in flight at panic time (timestamp ≤ boundary)
    // is dropped even though it arrives at the handler after the wipe.
    push(&mgr, "straggler", boundary).await;
    settle().await;
    assert!(
        mgr.recent_messages().await.is_empty(),
        "in-flight straggler must not survive the panic"
    );
}

#[tokio::test]
async fn straggler_emitted_during_teardown_is_never_emitted_to_ui() {
    // This is the test that actually pins the "boundary set BEFORE teardown"
    // fix. The ring is wiped at the end of the panic regardless, so a ring
    // assertion can't tell the two orderings apart — but the EMIT can. If
    // the boundary were stamped after teardown (the bug), the straggler the
    // connector emits during disconnect() would reach the handler with the
    // gate inactive and fire a `chat_message` event to the frontend, which
    // already cleared on the panic hotkey — repopulating the view.
    let emits = Arc::new(AtomicUsize::new(0));
    let sink: Arc<dyn EventSink> = Arc::new(CountingEventSink {
        chat_message_emits: emits.clone(),
    });
    let mgr = manager_with_sink(sink);

    // A message buffered just before panic — its timestamp predates the
    // boundary the panic will stamp.
    let straggler_ts = chrono::Utc::now().timestamp_millis();
    let connector: BoxedPlatform = Box::new(StragglerOnDisconnect {
        tx: mgr.message_tx.clone(),
        straggler_ts,
    });
    mgr.insert_test_connector(ChatPlatform::Twitch, connector)
        .await;

    mgr.disconnect_all("panic_triggered").await.unwrap();
    settle().await;

    assert_eq!(
        emits.load(Ordering::SeqCst),
        0,
        "a straggler emitted during panic teardown must never reach the UI"
    );
    assert!(
        mgr.recent_messages().await.is_empty(),
        "the ring must also be empty after the panic"
    );
}
