use crate::errors::{CoreError, ValidationIssue};
use chrono::Local;
use log::{error, info, warn};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};

fn chat_validation(code: &'static str, message: impl Into<String>) -> CoreError {
    CoreError::ValidationFailed {
        reasons: vec![ValidationIssue {
            code: code.into(),
            message: message.into(),
            path: None,
        }],
    }
}

use crate::models::{
    ChatConfig, ChatConnectionStatus, ChatMessage, ChatMessageDirection, ChatPlatform,
    ChatPlatformStatus, ChatSettings,
};
use crate::services::chat::{
    BoxedPlatform, StripchatConnector, TikTokConnector, TrovoConnector, TwitchConnector,
    YouTubeConnector,
};
use crate::services::EventSink;

/// Central manager for all chat platform connections
pub struct ChatManager {
    event_sink: Arc<dyn EventSink>,
    platforms: Arc<Mutex<HashMap<ChatPlatform, BoxedPlatform>>>,
    last_errors: Arc<Mutex<HashMap<ChatPlatform, String>>>,
    last_statuses: Arc<Mutex<HashMap<ChatPlatform, ChatConnectionStatus>>>,
    message_rx: Arc<Mutex<Option<mpsc::UnboundedReceiver<ChatMessage>>>>,
    message_tx: mpsc::UnboundedSender<ChatMessage>,
    log_tx: mpsc::UnboundedSender<ChatLogCommand>,
    log_session_start_ms: Arc<AtomicI64>,
    crosspost_enabled: Arc<AtomicBool>,
    send_enabled: Arc<Mutex<HashMap<ChatPlatform, bool>>>,
    chat_settings: Arc<Mutex<ChatSettings>>,
    /// `(enabled, salt_hex)` snapshot pushed from the
    /// active profile. When `enabled` is true, every inbound chat
    /// message's username is pseudonymised before it reaches the
    /// log writer (or the message-event stream). A salt-equipped
    /// frontend can decode by re-applying the HMAC.
    anonymous_policy: Arc<Mutex<Option<(bool, String)>>>,
}

impl ChatManager {
    /// Create a new ChatManager
    pub fn new(event_sink: Arc<dyn EventSink>, log_dir: PathBuf) -> Self {
        let (message_tx, message_rx) = mpsc::unbounded_channel();
        let (log_tx, log_rx) = mpsc::unbounded_channel();
        let log_session_start_ms = Arc::new(AtomicI64::new(0));

        let manager = Self {
            event_sink,
            platforms: Arc::new(Mutex::new(HashMap::new())),
            last_errors: Arc::new(Mutex::new(HashMap::new())),
            last_statuses: Arc::new(Mutex::new(HashMap::new())),
            message_rx: Arc::new(Mutex::new(Some(message_rx))),
            message_tx,
            log_tx,
            log_session_start_ms,
            crosspost_enabled: Arc::new(AtomicBool::new(false)),
            send_enabled: Arc::new(Mutex::new(HashMap::new())),
            chat_settings: Arc::new(Mutex::new(ChatSettings::default())),
            anonymous_policy: Arc::new(Mutex::new(None)),
        };

        // Start message handler
        manager.start_message_handler();
        manager.start_status_monitor();
        manager.start_log_writer(log_rx, log_dir);

        manager
    }

    /// Initialize a platform connector. Returns `None` for platforms
    /// SpiritStream has not yet implemented — `connect()` surfaces a
    /// clean validation error instead of fabricating a wrong-platform
    /// connector that would mis-route credentials.
    fn create_platform_connector(platform: ChatPlatform) -> Option<BoxedPlatform> {
        let boxed: BoxedPlatform = match platform {
            ChatPlatform::Twitch => Box::new(TwitchConnector::new()),
            ChatPlatform::TikTok => Box::new(TikTokConnector::new()),
            ChatPlatform::YouTube => Box::new(YouTubeConnector::new()),
            ChatPlatform::Trovo => Box::new(TrovoConnector::new()),
            ChatPlatform::Stripchat => Box::new(StripchatConnector::new()),
            ChatPlatform::Kick | ChatPlatform::Facebook => return None,
        };
        debug_assert_eq!(
            boxed.platform_name(),
            platform.as_str(),
            "Chat connector factory returned the wrong platform identity",
        );
        Some(boxed)
    }

    /// Connect to a chat platform
    pub async fn connect(&self, config: ChatConfig) -> Result<(), CoreError> {
        if !config.enabled {
            return Err(chat_validation(
                "chat_platform_not_enabled",
                "Platform is not enabled",
            ));
        }

        info!("Connecting to {} chat", config.platform.as_str());

        let mut platforms = self.platforms.lock().await;

        // Check if already connected
        if let Some(connector) = platforms.get(&config.platform) {
            if connector.is_connected() {
                return Err(chat_validation(
                    "chat_platform_already_connected",
                    format!("{} is already connected", config.platform.as_str()),
                ));
            }
        }

        // Create or get platform connector. Unimplemented platforms
        // (Kick, Facebook) return None → clean validation error.
        let mut connector = match platforms.remove(&config.platform) {
            Some(existing) => existing,
            None => Self::create_platform_connector(config.platform).ok_or_else(|| {
                chat_validation(
                    "chat_platform_unsupported",
                    format!(
                        "Chat for {} is not yet implemented",
                        config.platform.as_str()
                    ),
                )
            })?,
        };

        // Connect to the platform
        let message_tx = self.message_tx.clone();
        if let Err(e) = connector.connect(config.credentials, message_tx).await {
            let error = format!("Failed to connect to {}: {}", config.platform.as_str(), e);
            let mut last_errors = self.last_errors.lock().await;
            last_errors.insert(config.platform, error.clone());
            // Preserve the connector so status/errors can be surfaced in UI.
            platforms.insert(config.platform, connector);
            return Err(CoreError::Internal { context: error });
        }

        // Store the connector
        platforms.insert(config.platform, connector);

        // Clear last error on success
        let mut last_errors = self.last_errors.lock().await;
        last_errors.remove(&config.platform);

        info!("Successfully connected to {}", config.platform.as_str());
        Ok(())
    }

    /// Disconnect from a chat platform
    pub async fn disconnect(&self, platform: ChatPlatform) -> Result<(), CoreError> {
        info!("Disconnecting from {} chat", platform.as_str());

        let mut platforms = self.platforms.lock().await;

        if let Some(mut connector) = platforms.remove(&platform) {
            connector
                .disconnect()
                .await
                .map_err(|e| CoreError::Internal {
                    context: format!("Failed to disconnect from {}: {}", platform.as_str(), e),
                })?;

            // Re-insert the disconnected connector
            platforms.insert(platform, connector);

            let mut last_errors = self.last_errors.lock().await;
            last_errors.remove(&platform);

            info!("Successfully disconnected from {}", platform.as_str());
            Ok(())
        } else {
            Err(CoreError::ChatPlatformNotConnected {
                platform: platform.as_str().to_string(),
            })
        }
    }

    /// Update the OAuth access token for a specific platform (if connected).
    pub async fn update_platform_token(
        &self,
        platform: ChatPlatform,
        token: String,
    ) -> Result<(), CoreError> {
        let mut platforms = self.platforms.lock().await;

        if let Some(connector) = platforms.get_mut(&platform) {
            connector.update_token(token);
            Ok(())
        } else {
            Err(CoreError::ChatPlatformNotConnected {
                platform: platform.as_str().to_string(),
            })
        }
    }

    /// Send a chat message to the requested platforms.
    ///
    /// Enforces per-platform input length limits before dispatching
    /// (`ChatPlatform::max_message_chars`). Over-length messages return
    /// `Err("message exceeds {N}-char limit...")` per platform — never
    /// touch the wire. The PII filter runs after this check.
    pub async fn send_message(
        &self,
        message: String,
        platforms: &[ChatPlatform],
    ) -> Vec<(ChatPlatform, Result<(), CoreError>)> {
        let mut results = Vec::new();
        let message_chars = message.chars().count();
        let mut connectors = self.platforms.lock().await;

        for platform in platforms {
            let limit = platform.max_message_chars();
            if message_chars > limit {
                results.push((
                    *platform,
                    Err(CoreError::ChatMessageLengthExceeded {
                        platform: platform.as_str().to_string(),
                        limit,
                        actual: message_chars,
                    }),
                ));
                continue;
            }

            if let Some(connector) = connectors.get_mut(platform) {
                if !connector.can_send() {
                    results.push((
                        *platform,
                        Err(CoreError::ChatSendingDisabled {
                            platform: platform.as_str().to_string(),
                        }),
                    ));
                    continue;
                }

                let result = connector.send_message(message.clone()).await.map_err(|e| {
                    CoreError::Internal {
                        context: format!("Send failed: {}", e),
                    }
                });
                results.push((*platform, result));
            } else {
                results.push((
                    *platform,
                    Err(CoreError::ChatPlatformNotConnected {
                        platform: platform.as_str().to_string(),
                    }),
                ));
            }
        }

        results
    }

    /// Update the cached chat settings for the active profile.
    pub async fn update_profile_chat_settings(&self, settings: ChatSettings) {
        {
            let mut guard = self.chat_settings.lock().await;
            *guard = settings.clone();
        }

        self.set_crosspost_enabled(settings.crosspost_enabled);
        self.set_send_enabled(ChatPlatform::Twitch, settings.twitch_send_enabled)
            .await;
        self.set_send_enabled(ChatPlatform::YouTube, settings.youtube_send_enabled)
            .await;
        self.set_send_enabled(ChatPlatform::Trovo, settings.trovo_send_enabled)
            .await;
        self.set_send_enabled(ChatPlatform::Stripchat, settings.stripchat_send_enabled)
            .await;
    }

    /// Get the cached chat settings for the active profile.
    pub async fn profile_chat_settings(&self) -> ChatSettings {
        self.chat_settings.lock().await.clone()
    }

    /// Enable or disable crossposting of inbound messages.
    pub fn set_crosspost_enabled(&self, enabled: bool) {
        self.crosspost_enabled.store(enabled, Ordering::Relaxed);
    }

    /// Update per-platform send enable flags for crossposting.
    pub async fn set_send_enabled(&self, platform: ChatPlatform, enabled: bool) {
        let mut map = self.send_enabled.lock().await;
        map.insert(platform, enabled);
    }

    /// Disconnect from all platforms
    pub async fn disconnect_all(&self) -> Result<(), CoreError> {
        info!("Disconnecting from all chat platforms");

        let mut platforms = self.platforms.lock().await;
        let mut errors = Vec::new();

        for (platform, connector) in platforms.iter_mut() {
            if connector.is_connected() {
                if let Err(e) = connector.disconnect().await {
                    errors.push(format!("{}: {}", platform.as_str(), e));
                }
            }
        }

        if errors.is_empty() {
            let mut last_errors = self.last_errors.lock().await;
            last_errors.clear();
            info!("Successfully disconnected from all platforms");
            Ok(())
        } else {
            Err(CoreError::Internal {
                context: format!("Some disconnections failed: {}", errors.join(", ")),
            })
        }
    }

    /// Get status of all platforms
    pub async fn get_status(&self) -> Vec<ChatPlatformStatus> {
        let platforms = self.platforms.lock().await;
        let last_errors = self.last_errors.lock().await;

        platforms
            .iter()
            .map(|(platform, connector)| ChatPlatformStatus {
                platform: *platform,
                status: connector.status(),
                message_count: connector.message_count(),
                error: connector
                    .last_error()
                    .or_else(|| last_errors.get(platform).cloned()),
            })
            .collect()
    }

    /// Get status of a specific platform
    pub async fn get_platform_status(&self, platform: ChatPlatform) -> Option<ChatPlatformStatus> {
        let platforms = self.platforms.lock().await;
        let last_errors = self.last_errors.lock().await;

        platforms
            .get(&platform)
            .map(|connector| ChatPlatformStatus {
                platform,
                status: connector.status(),
                message_count: connector.message_count(),
                error: connector
                    .last_error()
                    .or_else(|| last_errors.get(&platform).cloned()),
            })
    }

    /// Check if any platform is connected
    pub async fn is_any_connected(&self) -> bool {
        let platforms = self.platforms.lock().await;
        platforms.values().any(|c| c.is_connected())
    }

    /// Start the message handler that forwards messages to the frontend
    fn start_message_handler(&self) {
        let event_sink = self.event_sink.clone();
        let message_rx = self.message_rx.clone();
        let log_tx = self.log_tx.clone();
        let platforms = self.platforms.clone();
        let crosspost_enabled = self.crosspost_enabled.clone();
        let send_enabled = self.send_enabled.clone();

        tokio::spawn(async move {
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

                    // Log message to disk (best effort, non-blocking)
                    let _ = log_tx.send(ChatLogCommand::Log(message.clone()));

                    // Emit message to frontend via EventSink
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
    }

    /// Monitor platform status transitions and emit connection events.
    fn start_status_monitor(&self) {
        let platforms = self.platforms.clone();
        let event_sink = self.event_sink.clone();
        let last_statuses = self.last_statuses.clone();

        tokio::spawn(async move {
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
                            let payload = serde_json::json!({
                                "platform": platform.as_str(),
                                "error": error.unwrap_or_else(|| "Connection lost".to_string()),
                            });
                            event_sink.emit("chat_connection_lost", payload);
                        }
                        if status == ChatConnectionStatus::Connected
                            && previous == Some(ChatConnectionStatus::Error)
                        {
                            let payload = serde_json::json!({
                                "platform": platform.as_str(),
                            });
                            event_sink.emit("chat_connection_restored", payload);
                        }
                    }
                }
            }
        });
    }

    /// Start the chat log writer background task.
    fn start_log_writer(
        &self,
        mut log_rx: mpsc::UnboundedReceiver<ChatLogCommand>,
        log_dir: PathBuf,
    ) {
        tokio::spawn(async move {
            let _ = std::fs::create_dir_all(&log_dir);
            let mut state = ChatLogState::new(log_dir);

            while let Some(cmd) = log_rx.recv().await {
                match cmd {
                    ChatLogCommand::StartSession => {
                        state.start_session();
                    }
                    ChatLogCommand::EndSession => {
                        state.end_session();
                    }
                    ChatLogCommand::Log(message) => {
                        state.write_message(&message);
                    }
                    ChatLogCommand::Flush(tx) => {
                        state.flush();
                        let _ = tx.send(());
                    }
                }
            }
        });
    }

    /// Start a new log session (stream start).
    pub fn start_log_session(&self) {
        let now = chrono::Local::now().timestamp_millis();
        self.log_session_start_ms.store(now, Ordering::Relaxed);
        let _ = self.log_tx.send(ChatLogCommand::StartSession);
    }

    /// End the current log session (stream end).
    pub fn end_log_session(&self) {
        self.log_session_start_ms.store(0, Ordering::Relaxed);
        let _ = self.log_tx.send(ChatLogCommand::EndSession);
    }

    /// Push the active profile's anonymous-mode policy.
    /// Transports call this on profile activate so subsequent chat
    /// messages logged through [`log_message`] get pseudonymised
    /// transparently.
    pub async fn set_anonymous_policy(&self, enabled: bool, salt_hex: String) {
        let mut guard = self.anonymous_policy.lock().await;
        *guard = Some((enabled, salt_hex));
    }

    /// Clear the active anonymous-mode policy. Called when
    /// the user deactivates a profile or revokes the policy outright.
    pub async fn clear_anonymous_policy(&self) {
        let mut guard = self.anonymous_policy.lock().await;
        *guard = None;
    }

    /// Log a message to disk (best effort). Applies the
    /// anonymous-mode pseudonymizer to the username field before
    /// queueing, so the log writer never sees plaintext usernames
    /// when anonymous mode is on.
    pub fn log_message(&self, message: ChatMessage) {
        let message = self.apply_anonymous_policy_to_message(message);
        let _ = self.log_tx.send(ChatLogCommand::Log(message));
    }

    /// Hot path helper: apply the cached anonymous-mode policy to a
    /// `ChatMessage` if active. Uses a blocking lock (`try_lock`)
    /// because `log_message` is non-async by contract — if the policy
    /// mutex is contended at this exact moment, we let the message
    /// through unchanged rather than blocking. Frequency: contention
    /// only at profile-activate-time, which is human-paced.
    fn apply_anonymous_policy_to_message(&self, mut message: ChatMessage) -> ChatMessage {
        if let Ok(guard) = self.anonymous_policy.try_lock() {
            if let Some((enabled, salt)) = guard.as_ref() {
                if *enabled && !salt.is_empty() {
                    message.username =
                        crate::services::pseudonymizer::pseudonymize(&message.username, salt);
                }
            }
        }
        message
    }

    /// Flush pending log writes to disk.
    pub async fn flush_chat_logs(&self) -> Result<(), CoreError> {
        let (tx, rx) = oneshot::channel();
        self.log_tx
            .send(ChatLogCommand::Flush(tx))
            .map_err(|_| CoreError::Internal {
                context: "Chat log writer is not available".into(),
            })?;
        rx.await.map_err(|_| CoreError::Internal {
            context: "Failed to flush chat logs".into(),
        })
    }

    /// Return the current log session start timestamp (ms), if active.
    pub fn log_session_start_ms(&self) -> Option<i64> {
        let value = self.log_session_start_ms.load(Ordering::Relaxed);
        if value > 0 {
            Some(value)
        } else {
            None
        }
    }
}

/// Cleanup implementation
impl Drop for ChatManager {
    fn drop(&mut self) {
        info!("ChatManager dropped");
    }
}

// ============================================================================
// Chat Log Writer
// ============================================================================

enum ChatLogCommand {
    StartSession,
    EndSession,
    Log(ChatMessage),
    Flush(oneshot::Sender<()>),
}

struct ChatLogState {
    log_dir: PathBuf,
    active: bool,
    current_hour_key: Option<String>,
    writer: Option<BufWriter<File>>,
}

impl ChatLogState {
    fn new(log_dir: PathBuf) -> Self {
        Self {
            log_dir,
            active: false,
            current_hour_key: None,
            writer: None,
        }
    }

    fn start_session(&mut self) {
        self.active = true;
        self.current_hour_key = None;
        self.writer = None;
    }

    fn end_session(&mut self) {
        self.active = false;
        self.flush();
        self.writer = None;
        self.current_hour_key = None;
    }

    fn write_message(&mut self, message: &ChatMessage) {
        if !self.active {
            return;
        }

        let hour_key = Local::now().format("%Y%m%d-%H").to_string();
        if self.current_hour_key.as_deref() != Some(&hour_key) {
            if let Err(e) = self.rotate_file(&hour_key) {
                warn!("Failed to rotate chat log file: {}", e);
                return;
            }
        }

        if let Some(writer) = self.writer.as_mut() {
            if let Ok(line) = serde_json::to_string(message) {
                if let Err(e) = writer.write_all(line.as_bytes()) {
                    warn!("Failed to write chat log line: {}", e);
                    return;
                }
                let _ = writer.write_all(b"\n");
            }
        }
    }

    fn rotate_file(&mut self, hour_key: &str) -> Result<(), CoreError> {
        let path = self.log_dir.join(format!("chatlog_{}.jsonl", hour_key));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| CoreError::Internal {
                context: format!("Failed to open chat log file: {e}"),
            })?;

        self.writer = Some(BufWriter::new(file));
        self.current_hour_key = Some(hour_key.to_string());
        Ok(())
    }

    fn flush(&mut self) {
        if let Some(writer) = self.writer.as_mut() {
            let _ = writer.flush();
        }
    }
}

#[cfg(test)]
mod send_message_tests {
    use super::*;
    use crate::errors::CoreError;
    use crate::services::events::NoopEventSink;

    fn manager() -> ChatManager {
        let event_sink: Arc<dyn EventSink> = Arc::new(NoopEventSink);
        ChatManager::new(
            event_sink,
            std::env::temp_dir().join("spiritstream-chat-test"),
        )
    }

    /// Plan-cited variant: messages over the per-platform char limit must
    /// fail with `ChatMessageLengthExceeded` BEFORE the wire / PII filter.
    #[tokio::test]
    async fn over_length_message_returns_chat_message_length_exceeded() {
        let mgr = manager();
        let limit = ChatPlatform::Twitch.max_message_chars();
        let too_long = "x".repeat(limit + 50);
        let results = mgr.send_message(too_long, &[ChatPlatform::Twitch]).await;
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
        let mgr = manager();
        let results = mgr
            .send_message("hello".to_string(), &[ChatPlatform::Twitch])
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
        let mgr = manager();
        let results = mgr
            .send_message(
                "ok".to_string(),
                &[ChatPlatform::Twitch, ChatPlatform::YouTube],
            )
            .await;
        assert_eq!(results.len(), 2);
        for (_, r) in &results {
            assert!(matches!(r, Err(CoreError::ChatPlatformNotConnected { .. })));
        }
    }
}
