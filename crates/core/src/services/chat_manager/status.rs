use std::sync::atomic::Ordering;
use std::sync::Arc;

use log::{error, info, warn};
use serde_json::json;

use crate::errors::CoreError;
use crate::models::{ChatConnectionStatus, ChatMessage, ChatMessageDirection};

use super::log_writer::ChatLogCommand;

/// Apply the cached anonymous-mode policy to a message arriving on the
/// inbound path. Mirrors `ChatManager::apply_anonymous_policy_to_message`
/// but takes the policy `Arc` directly so it can run inside the
/// spawned message-handler task (where `&self` is not available).
///
/// Fails (→ caller DROPS the message) when anonymous mode is enabled
/// but the salt can't pseudonymise. The old behavior passed the
/// plaintext username through — anonymous mode silently off while the
/// UI said it was on.
fn apply_anonymous_policy_inbound(
    policy: &Arc<std::sync::RwLock<Option<(bool, String)>>>,
    mut message: ChatMessage,
) -> Result<ChatMessage, CoreError> {
    let guard = policy.read().unwrap_or_else(|e| e.into_inner());
    if let Some((enabled, salt)) = guard.as_ref() {
        if *enabled {
            message.username =
                crate::services::pseudonymizer::pseudonymize(&message.username, salt)?;
        }
    }
    Ok(message)
}

impl super::ChatManager {
    /// Start the message handler that forwards messages to the frontend.
    pub(super) fn start_message_handler(&self) {
        let event_sink = self.event_sink.clone();
        let message_rx = self.message_rx.clone();
        let log_tx = self.log_tx.clone();
        let platforms = self.platforms.clone();
        let crosspost_enabled = self.crosspost_enabled.clone();
        let send_enabled = self.send_enabled.clone();
        let anonymous_policy = self.anonymous_policy.clone();

        let handle = tokio::spawn(async move {
            use std::collections::{HashSet, VecDeque};
            const MAX_SEEN_IDS: usize = 5000;
            let mut seen_ids: HashSet<String> = HashSet::new();
            let mut seen_order: VecDeque<String> = VecDeque::new();

            let rx = {
                let mut guard = message_rx.lock().await;
                guard.take()
            };

            if let Some(mut receiver) = rx {
                info!("Chat message handler started");

                while let Some(message) = receiver.recv().await {
                    if seen_ids.contains(&message.id) {
                        continue;
                    }
                    seen_ids.insert(message.id.clone());
                    seen_order.push_back(message.id.clone());
                    if seen_order.len() > MAX_SEEN_IDS {
                        if let Some(old) = seen_order.pop_front() {
                            seen_ids.remove(&old);
                        }
                    }

                    // Anonymous-mode policy is applied on the inbound boundary
                    // BEFORE the message reaches the log writer, the event
                    // bus, or the crosspost path. Pre-F2 only the log writer
                    // pseudonymised on its own — the frontend event stream
                    // leaked plaintext usernames whenever anonymous mode was
                    // active. See `feedback_intentional_security.md`.
                    //
                    // Fail-safe direction: a message that CANNOT be
                    // pseudonymised is dropped, never forwarded with its
                    // real username.
                    let message =
                        match apply_anonymous_policy_inbound(&anonymous_policy, message) {
                            Ok(m) => m,
                            Err(e) => {
                                error!(
                                    "anonymous mode active but pseudonymization failed; \
                                     dropping inbound chat message: {e}"
                                );
                                event_sink.emit(
                                    "anonymous_mode_error",
                                    json!({ "reason": "salt_invalid", "dropped": true }),
                                );
                                continue;
                            }
                        };

                    // Log message to disk (best effort). We're in an async
                    // context inside the manager's message-handler task, so
                    // awaiting on a full log channel is correct back-pressure
                    // — preferable to dropping log entries.
                    let _ = log_tx
                        .send(ChatLogCommand::Log(Box::new(message.clone())))
                        .await;

                    // Emit message to frontend via EventSink.
                    if let Ok(payload) = serde_json::to_value(&message) {
                        event_sink.emit("chat_message", payload);
                    } else {
                        error!("Failed to serialize chat message");
                    }

                    // Crosspost inbound messages to other enabled platforms.
                    if message.direction == ChatMessageDirection::Inbound
                        && crosspost_enabled.load(Ordering::Relaxed)
                    {
                        let origin = message.platform;
                        let text = message.message.clone();
                        let targets = {
                            let enabled = send_enabled.lock().await;
                            let connectors = platforms.lock().await;
                            let mut list = Vec::new();

                            for (platform, connector) in connectors.iter() {
                                if *platform == origin {
                                    continue;
                                }
                                if !connector.can_send() {
                                    continue;
                                }
                                if !enabled.get(platform).copied().unwrap_or(false) {
                                    continue;
                                }
                                list.push(*platform);
                            }

                            list
                        };

                        if !targets.is_empty() {
                            let platforms = platforms.clone();
                            tokio::spawn(async move {
                                let mut connectors = platforms.lock().await;
                                for platform in targets {
                                    if let Some(connector) = connectors.get_mut(&platform) {
                                        if let Err(err) = connector.send_message(text.clone()).await
                                        {
                                            warn!(
                                                "Crosspost to {} failed: {}",
                                                platform.as_str(),
                                                err
                                            );
                                        }
                                    }
                                }
                            });
                        }
                    }
                }

                info!("Chat message handler stopped");
            } else {
                error!("Message receiver was already taken");
            }
        });
        if let Ok(mut slot) = self.message_handler_handle.lock() {
            // Replacing the slot drops the previous handle (if any) — its
            // task already exited when its receiver returned None, so
            // there's nothing to abort.
            *slot = Some(handle);
        }
    }

    /// Monitor platform status transitions and emit connection events.
    pub(super) fn start_status_monitor(&self) {
        let platforms = self.platforms.clone();
        let event_sink = self.event_sink.clone();
        let last_statuses = self.last_statuses.clone();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

            loop {
                interval.tick().await;

                let snapshots = {
                    let platforms_guard = platforms.lock().await;
                    platforms_guard
                        .iter()
                        .map(|(platform, connector)| {
                            (*platform, connector.status(), connector.last_error())
                        })
                        .collect::<Vec<_>>()
                };

                let mut last = last_statuses.lock().await;

                for (platform, status, error) in snapshots {
                    let previous = last.get(&platform).copied();
                    if previous != Some(status) {
                        last.insert(platform, status);

                        if status == ChatConnectionStatus::Error {
                            let payload = json!({
                                "platform": platform.as_str(),
                                "error": error.unwrap_or_else(|| "Connection lost".to_string()),
                            });
                            event_sink.emit("chat_connection_lost", payload);
                        }
                        if status == ChatConnectionStatus::Connected
                            && previous == Some(ChatConnectionStatus::Error)
                        {
                            let payload = json!({
                                "platform": platform.as_str(),
                            });
                            event_sink.emit("chat_connection_restored", payload);
                        }
                    }
                }
            }
        });
        if let Ok(mut slot) = self.status_monitor_handle.lock() {
            *slot = Some(handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::apply_anonymous_policy_inbound;
    use crate::models::{ChatMessage, ChatPlatform};
    use std::sync::{Arc, RwLock};

    fn sample_message(username: &str) -> ChatMessage {
        ChatMessage::new(ChatPlatform::Twitch, username.into(), "hi".into())
    }

    /// F2 regression: anonymous mode must rewrite inbound usernames
    /// before they reach the log writer or event bus. Pre-F2 the
    /// `start_message_handler` path bypassed the pseudonymiser; only
    /// the outbound `log_message` API rewrote.
    ///
    /// Q10: also pins `pseudonymizer::looks_pseudonymized` (the format
    /// detector the chat-log viewer UI uses to decide whether to show
    /// a "decode" affordance). Asserting `looks_pseudonymized(...)`
    /// here gives the format-detector a real consumer + a regression
    /// guard against the prefix/length shape drifting.
    #[tokio::test]
    async fn anonymous_policy_pseudonymises_inbound_username() {
        let policy: Arc<RwLock<Option<(bool, String)>>> =
            Arc::new(RwLock::new(Some((true, "deadbeef".into()))));
        let original = sample_message("RealUser");
        let rewritten = apply_anonymous_policy_inbound(&policy, original).unwrap();
        assert_ne!(rewritten.username, "RealUser");
        assert!(!rewritten.username.is_empty());
        assert!(
            crate::services::pseudonymizer::looks_pseudonymized(&rewritten.username),
            "pseudonymised username must match the format detector: {}",
            rewritten.username,
        );
    }

    /// Anonymous-mode off → username passes through unchanged.
    #[tokio::test]
    async fn anonymous_policy_inactive_leaves_username_untouched() {
        let policy: Arc<RwLock<Option<(bool, String)>>> =
            Arc::new(RwLock::new(Some((false, "deadbeef".into()))));
        let original = sample_message("PublicHandle");
        let result = apply_anonymous_policy_inbound(&policy, original).unwrap();
        assert_eq!(result.username, "PublicHandle");
        // Bare plaintext should NOT look pseudonymised — the inverse
        // shape check.
        assert!(!crate::services::pseudonymizer::looks_pseudonymized(
            &result.username
        ));
    }

    /// No policy set → username passes through unchanged.
    #[tokio::test]
    async fn anonymous_policy_unset_leaves_username_untouched() {
        let policy: Arc<RwLock<Option<(bool, String)>>> = Arc::new(RwLock::new(None));
        let original = sample_message("NoPolicy");
        let result = apply_anonymous_policy_inbound(&policy, original).unwrap();
        assert_eq!(result.username, "NoPolicy");
    }

    /// Fail-safe direction: anonymous mode enabled + broken salt must
    /// be an error (caller drops the message), never a plaintext
    /// pass-through. Pre-fix this returned the message unchanged.
    #[tokio::test]
    async fn anonymous_policy_with_broken_salt_errors_instead_of_leaking() {
        for bad_salt in ["", "not-hex"] {
            let policy: Arc<RwLock<Option<(bool, String)>>> =
                Arc::new(RwLock::new(Some((true, bad_salt.into()))));
            let original = sample_message("RealUser");
            assert!(
                apply_anonymous_policy_inbound(&policy, original).is_err(),
                "salt {bad_salt:?} must fail loud, not leak the username"
            );
        }
    }

    use crate::services::chat_manager::ChatManager;
    use crate::services::EventSink;
    use std::sync::Mutex as StdMutex;
    use tempfile::TempDir;

    #[derive(Default)]
    struct RecordingSink {
        events: StdMutex<Vec<(String, serde_json::Value)>>,
    }

    impl EventSink for RecordingSink {
        fn emit(&self, event: &str, payload: serde_json::Value) {
            self.events
                .lock()
                .unwrap()
                .push((event.to_string(), payload));
        }
    }

    impl RecordingSink {
        fn chat_messages(&self) -> Vec<serde_json::Value> {
            self.events
                .lock()
                .unwrap()
                .iter()
                .filter(|(e, _)| e == "chat_message")
                .map(|(_, p)| p.clone())
                .collect()
        }
    }

    /// Poll the sink until it has at least `n` chat_message events or the
    /// budget runs out. The handler runs on a spawned task, so the emit is
    /// not synchronous with the `send`.
    async fn wait_for_chat_messages(sink: &Arc<RecordingSink>, n: usize) -> Vec<serde_json::Value> {
        for _ in 0..100 {
            let msgs = sink.chat_messages();
            if msgs.len() >= n {
                return msgs;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        sink.chat_messages()
    }

    fn manager_with_sink() -> (ChatManager, Arc<RecordingSink>, TempDir) {
        let dir = TempDir::new().unwrap();
        let sink = Arc::new(RecordingSink::default());
        let event_sink: Arc<dyn EventSink> = sink.clone();
        let manager = ChatManager::new(event_sink, dir.path().to_path_buf());
        (manager, sink, dir)
    }

    #[tokio::test]
    async fn message_handler_emits_inbound_message_to_event_sink() {
        let (manager, sink, _dir) = manager_with_sink();
        manager
            .message_tx
            .send(sample_message("Viewer42"))
            .await
            .unwrap();

        let msgs = wait_for_chat_messages(&sink, 1).await;
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["username"], "Viewer42");
    }

    #[tokio::test]
    async fn message_handler_deduplicates_by_id() {
        let (manager, sink, _dir) = manager_with_sink();
        let msg = sample_message("Dup");
        // Same id sent twice — the handler's seen-id set must drop the second.
        manager.message_tx.send(msg.clone()).await.unwrap();
        manager.message_tx.send(msg).await.unwrap();

        let msgs = wait_for_chat_messages(&sink, 1).await;
        // Give the second (deduped) send a chance to be wrongly emitted.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(
            sink.chat_messages().len(),
            1,
            "duplicate id must be dropped"
        );
        assert_eq!(msgs[0]["id"], sink.chat_messages()[0]["id"]);
    }

    #[tokio::test]
    async fn message_handler_pseudonymises_when_anonymous_mode_active() {
        let (manager, sink, _dir) = manager_with_sink();
        *manager.anonymous_policy.write().unwrap() = Some((true, "deadbeef".into()));
        manager
            .message_tx
            .send(sample_message("RealName"))
            .await
            .unwrap();

        let msgs = wait_for_chat_messages(&sink, 1).await;
        assert_eq!(msgs.len(), 1);
        let emitted = msgs[0]["username"].as_str().unwrap();
        assert_ne!(
            emitted, "RealName",
            "anonymous mode must rewrite the username"
        );
        assert!(crate::services::pseudonymizer::looks_pseudonymized(emitted));
    }
}
