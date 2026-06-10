//! Central manager for all chat platform connections.
//!
//! Method impls live in focused submodules:
//! - `connection`: connect / disconnect / update_platform_token / disconnect_all.
//! - `send`: send_message (PII gate + per-platform char-limit + audit).
//! - `status`: get_status / is_any_connected + background message-handler
//!   and status-monitor tasks.
//! - `settings`: profile chat settings cache + per-platform send-enable
//!   flags + anonymous-mode policy.
//! - `log_writer`: chat log session lifecycle + per-hour rotation +
//!   ChatLogState + ChatLogCommand.

mod connection;
mod log_writer;
mod send;
mod settings;
mod status;

#[cfg(test)]
mod send_message_tests;

use log_writer::ChatLogCommand;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use crate::errors::{CoreError, ValidationIssue};
use crate::models::{
    ChatConnectionStatus, ChatMessage, ChatPlatform, ChatPlatformStatus, ChatSettings,
};
use crate::services::chat::{
    BoxedPlatform, ChatEndpoints, FacebookConnector, KickConnector, TikTokConnector,
    TrovoConnector, TwitchConnector, YouTubeConnector,
};
use crate::services::{AuditLogService, EventSink};

pub(super) fn chat_validation(code: &'static str, message: impl Into<String>) -> CoreError {
    CoreError::ValidationFailed {
        reasons: vec![ValidationIssue {
            code: code.into(),
            message: message.into(),
            path: None,
        }],
    }
}

/// Central manager for all chat platform connections
pub struct ChatManager {
    pub(super) event_sink: Arc<dyn EventSink>,
    pub(super) platforms: Arc<Mutex<HashMap<ChatPlatform, BoxedPlatform>>>,
    pub(super) last_statuses: Arc<Mutex<HashMap<ChatPlatform, ChatConnectionStatus>>>,
    pub(super) message_rx: Arc<Mutex<Option<mpsc::Receiver<ChatMessage>>>>,
    pub(super) message_tx: mpsc::Sender<ChatMessage>,
    pub(super) log_tx: mpsc::Sender<ChatLogCommand>,
    pub(super) log_session_start_ms: Arc<AtomicI64>,
    pub(super) crosspost_enabled: Arc<AtomicBool>,
    /// JoinHandles for the long-lived background tasks
    /// (`message_handler`, `status_monitor`, `log_writer`). Stored so
    /// `Drop` can `abort()` them on graceful shutdown — pre-fix the
    /// tasks ran forever until tokio runtime shutdown, which leaked
    /// the work each time a `ChatManager` was rebuilt (e.g. during
    /// profile reactivation in tests). Sprint I1 / O.8f pattern.
    pub(super) message_handler_handle: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    pub(super) status_monitor_handle: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    pub(super) log_writer_handle: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    pub(super) send_enabled: Arc<Mutex<HashMap<ChatPlatform, bool>>>,
    pub(super) chat_settings: Arc<Mutex<ChatSettings>>,
    /// `(enabled, salt_hex)` snapshot pushed from the
    /// active profile. When `enabled` is true, every inbound chat
    /// message's username is pseudonymised before it reaches the
    /// log writer (or the message-event stream). A salt-equipped
    /// frontend can decode by re-applying the HMAC.
    ///
    /// `std::sync::RwLock`, not an async mutex: readers are the
    /// per-message hot paths (never blocked in practice), the only
    /// writer is profile activation (human-paced). The previous async
    /// mutex forced the sync log path through `try_lock`, whose
    /// contention fallback wrote PLAINTEXT usernames to disk — a
    /// silent-fallback hole this lock choice removes outright.
    pub(super) anonymous_policy: Arc<std::sync::RwLock<Option<(bool, String)>>>,
    /// Audit-log handle, wired post-construction by `ServiceRegistry`
    /// (mirrors the `ThemeManager::set_audit_log` pattern — keeps
    /// construction order acyclic). Used for chat-mutation entries:
    /// `ChatMessageSent`, `ChatPlatformConnected`,
    /// `ChatPlatformDisconnected`. Before the registry wires it,
    /// chat mutations succeed silently (no panic, no audit entry) —
    /// matches `ThemeManager`'s degraded-mode behavior.
    pub(super) audit_log: Arc<std::sync::RwLock<Option<Arc<AuditLogService>>>>,
    /// Network endpoints handed to every connector at construction.
    /// Production defaults in non-test builds; the connector integration
    /// harness swaps in mock-server origins via `ChatEndpoints::for_mock`.
    /// Deliberately not env-overridable — redirecting chat/OAuth traffic
    /// at runtime would be an exfiltration vector (see `endpoints.rs`).
    pub(super) chat_endpoints: ChatEndpoints,
}

impl ChatManager {
    /// Create a new ChatManager with the production network endpoints.
    pub fn new(event_sink: Arc<dyn EventSink>, log_dir: PathBuf) -> Self {
        Self::with_endpoints(event_sink, log_dir, ChatEndpoints::default())
    }

    /// Create a ChatManager with caller-supplied connector endpoints.
    /// The only non-default caller is the connector integration harness,
    /// which injects `ChatEndpoints::for_mock` so connectors dial local
    /// mock servers instead of the real platforms.
    pub(crate) fn with_endpoints(
        event_sink: Arc<dyn EventSink>,
        log_dir: PathBuf,
        chat_endpoints: ChatEndpoints,
    ) -> Self {
        // Bounded channels prevent OOM under pathological chat flood
        // (Twitch raid, misbehaving connector, etc). Sizes are generous
        // enough that normal load never touches them — buffering ~33s
        // of messages at 30 msg/s on the message channel, and ~166s of
        // writes at 30 cmd/s on the log channel. See `ChatPlatform`
        // trait docstring for the drop-on-Full / break-on-Closed
        // contract callers must follow.
        const MESSAGE_CHANNEL_CAPACITY: usize = 1000;
        const LOG_CHANNEL_CAPACITY: usize = 5000;
        let (message_tx, message_rx) = mpsc::channel(MESSAGE_CHANNEL_CAPACITY);
        let (log_tx, log_rx) = mpsc::channel(LOG_CHANNEL_CAPACITY);
        let log_session_start_ms = Arc::new(AtomicI64::new(0));

        let manager = Self {
            event_sink,
            platforms: Arc::new(Mutex::new(HashMap::new())),
            last_statuses: Arc::new(Mutex::new(HashMap::new())),
            message_rx: Arc::new(Mutex::new(Some(message_rx))),
            message_tx,
            log_tx,
            log_session_start_ms,
            crosspost_enabled: Arc::new(AtomicBool::new(false)),
            send_enabled: Arc::new(Mutex::new(HashMap::new())),
            chat_settings: Arc::new(Mutex::new(ChatSettings::default())),
            anonymous_policy: Arc::new(std::sync::RwLock::new(None)),
            audit_log: Arc::new(std::sync::RwLock::new(None)),
            message_handler_handle: std::sync::Mutex::new(None),
            status_monitor_handle: std::sync::Mutex::new(None),
            log_writer_handle: std::sync::Mutex::new(None),
            chat_endpoints,
        };

        // Start message handler.
        manager.start_message_handler();
        manager.start_status_monitor();
        manager.start_log_writer(log_rx, log_dir);

        manager
    }

    /// Wire the audit-log handle post-construction. Called by
    /// `crates/core/src/registry.rs` after both services exist.
    /// Idempotent — a second call replaces the stored handle.
    pub fn set_audit_log(&self, audit: Arc<AuditLogService>) {
        match self.audit_log.write() {
            Ok(mut slot) => *slot = Some(audit),
            Err(e) => {
                log::error!("chat_manager audit_log write lock poisoned during set_audit_log: {e}")
            }
        }
    }

    /// Read the wired audit-log handle (clone of the Arc). Returns
    /// `None` before `set_audit_log` runs — chat mutations then
    /// proceed without an audit entry, matching the `ThemeManager`
    /// degraded-mode behavior. A poisoned lock is *not* the same as
    /// "unwired" — surface it via `error!` so audit gaps caused by an
    /// upstream panic aren't silent (see feedback_no_fallback_streaming).
    pub(super) fn audit(&self) -> Option<Arc<AuditLogService>> {
        match self.audit_log.read() {
            Ok(g) => g.clone(),
            Err(e) => {
                log::error!("chat_manager audit_log read lock poisoned — audit entry dropped: {e}");
                None
            }
        }
    }

    /// Get status of all platforms. `connector.last_error()` is the single
    /// source — every implemented connector tracks its own error state in
    /// `self.last_error`; the trait default returns `None` for unimplemented
    /// connectors. The prior stale `last_errors` cache was redundant and
    /// surfaced stale errors after a connector recovered.
    pub async fn get_status(&self) -> Vec<ChatPlatformStatus> {
        let platforms = self.platforms.lock().await;
        platforms
            .iter()
            .map(|(platform, connector)| ChatPlatformStatus {
                platform: *platform,
                status: connector.status(),
                message_count: connector.message_count(),
                error: connector.last_error(),
            })
            .collect()
    }

    /// Get status of a specific platform.
    pub async fn get_platform_status(&self, platform: ChatPlatform) -> Option<ChatPlatformStatus> {
        let platforms = self.platforms.lock().await;
        platforms
            .get(&platform)
            .map(|connector| ChatPlatformStatus {
                platform,
                status: connector.status(),
                message_count: connector.message_count(),
                error: connector.last_error(),
            })
    }

    /// Check if any platform is connected.
    pub async fn is_any_connected(&self) -> bool {
        let platforms = self.platforms.lock().await;
        platforms.values().any(|c| c.is_connected())
    }

    /// Initialize a platform connector. Returns `None` for platforms
    /// SpiritStream has not yet implemented — `connect()` surfaces a
    /// clean validation error instead of fabricating a wrong-platform
    /// connector that would mis-route credentials.
    /// Build the connector for a `ChatPlatform` variant. Now total —
    /// every variant maps to a real connector, no more `None` arms.
    /// The signature stays `Option<BoxedPlatform>` for backward
    /// compatibility with the call site, and a future "platform was
    /// removed pending re-implementation" branch can return `None`
    /// again without re-touching every caller.
    pub(super) fn create_platform_connector(
        platform: ChatPlatform,
        endpoints: &ChatEndpoints,
    ) -> Option<BoxedPlatform> {
        let boxed: BoxedPlatform = match platform {
            // Twitch's HTTP seams (GQL lookup + token validate) take the
            // injected endpoints; the IRC ride stays inside `twitch-irc`,
            // which exposes no server-address override (see endpoints.rs).
            ChatPlatform::Twitch => Box::new(TwitchConnector::with_endpoints(endpoints)),
            // TikTok lives inside `piratetok-live-rs`, whose builder takes
            // only a username — no endpoint to inject (see endpoints.rs).
            ChatPlatform::TikTok => Box::new(TikTokConnector::new()),
            ChatPlatform::YouTube => Box::new(YouTubeConnector::with_endpoints(endpoints)),
            ChatPlatform::Trovo => Box::new(TrovoConnector::with_endpoints(endpoints)),
            ChatPlatform::Kick => Box::new(KickConnector::with_endpoints(endpoints)),
            ChatPlatform::Facebook => Box::new(FacebookConnector::with_endpoints(endpoints)),
        };
        debug_assert_eq!(
            boxed.platform_name(),
            platform.as_str(),
            "Chat connector factory returned the wrong platform identity",
        );
        Some(boxed)
    }
}

/// Cleanup implementation. Aborts the long-lived background tasks
/// (`message_handler`, `status_monitor`, `log_writer`) so they don't
/// linger after their owning `ChatManager` is dropped — pre-fix the
/// tasks ran forever until tokio runtime shutdown, which leaked work
/// every time a `ChatManager` was rebuilt (e.g. during profile
/// reactivation in tests). Sprint O.8f.
impl Drop for ChatManager {
    fn drop(&mut self) {
        for slot in [
            &self.message_handler_handle,
            &self.status_monitor_handle,
            &self.log_writer_handle,
        ] {
            if let Ok(mut guard) = slot.lock() {
                if let Some(handle) = guard.take() {
                    handle.abort();
                }
            }
        }
        log::info!("ChatManager dropped");
    }
}
