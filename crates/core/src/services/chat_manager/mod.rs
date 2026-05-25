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
    BoxedPlatform, StripchatConnector, TikTokConnector, TrovoConnector, TwitchConnector,
    YouTubeConnector,
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
    pub(super) message_rx: Arc<Mutex<Option<mpsc::UnboundedReceiver<ChatMessage>>>>,
    pub(super) message_tx: mpsc::UnboundedSender<ChatMessage>,
    pub(super) log_tx: mpsc::UnboundedSender<ChatLogCommand>,
    pub(super) log_session_start_ms: Arc<AtomicI64>,
    pub(super) crosspost_enabled: Arc<AtomicBool>,
    pub(super) send_enabled: Arc<Mutex<HashMap<ChatPlatform, bool>>>,
    pub(super) chat_settings: Arc<Mutex<ChatSettings>>,
    /// `(enabled, salt_hex)` snapshot pushed from the
    /// active profile. When `enabled` is true, every inbound chat
    /// message's username is pseudonymised before it reaches the
    /// log writer (or the message-event stream). A salt-equipped
    /// frontend can decode by re-applying the HMAC.
    pub(super) anonymous_policy: Arc<Mutex<Option<(bool, String)>>>,
    /// Audit-log handle, wired post-construction by `ServiceRegistry`
    /// (mirrors the `ThemeManager::set_audit_log` pattern — keeps
    /// construction order acyclic). Used for chat-mutation entries:
    /// `ChatMessageSent`, `ChatPlatformConnected`,
    /// `ChatPlatformDisconnected`. Before the registry wires it,
    /// chat mutations succeed silently (no panic, no audit entry) —
    /// matches `ThemeManager`'s degraded-mode behavior.
    pub(super) audit_log: Arc<std::sync::RwLock<Option<Arc<AuditLogService>>>>,
}

impl ChatManager {
    /// Create a new ChatManager.
    pub fn new(event_sink: Arc<dyn EventSink>, log_dir: PathBuf) -> Self {
        let (message_tx, message_rx) = mpsc::unbounded_channel();
        let (log_tx, log_rx) = mpsc::unbounded_channel();
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
            anonymous_policy: Arc::new(Mutex::new(None)),
            audit_log: Arc::new(std::sync::RwLock::new(None)),
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
        if let Ok(mut slot) = self.audit_log.write() {
            *slot = Some(audit);
        }
    }

    /// Read the wired audit-log handle (clone of the Arc). Returns
    /// `None` before `set_audit_log` runs — chat mutations then
    /// proceed without an audit entry, matching the `ThemeManager`
    /// degraded-mode behavior.
    pub(super) fn audit(&self) -> Option<Arc<AuditLogService>> {
        self.audit_log.read().ok().and_then(|g| g.clone())
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
        platforms.get(&platform).map(|connector| ChatPlatformStatus {
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
    pub(super) fn create_platform_connector(platform: ChatPlatform) -> Option<BoxedPlatform> {
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
}

/// Cleanup implementation
impl Drop for ChatManager {
    fn drop(&mut self) {
        log::info!("ChatManager dropped");
    }
}
