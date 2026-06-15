//! TikTok Live chat connector — read-only.
//!
//! TikTok publishes no official chat API; this connector uses the
//! community `piratetok-live-rs` crate (protobuf-over-WebSocket
//! reverse-engineered from the TikTok web client). No auth, no API
//! key required — connect by username and receive realtime events.
//!
//! **Send is intentionally disabled.** TikTok rejects third-party
//! chat-send for non-creator-app integrations, and the
//! reverse-engineered protocol does not expose a verified send path.
//! `can_send()` returns `false`; `send_message()` returns
//! `PlatformError::Platform("TikTok chat is read-only ...")`.
//!
//! **Stability caveat.** When TikTok rotates the protobuf protocol,
//! events stop arriving until the upstream crate publishes a fix and
//! we bump the dep. The connector surfaces the disconnect as
//! `ChatConnectionStatus::Error` with a descriptive last_error so the
//! UI doesn't silently hide it. Bumping the dep is a maintainer
//! ritual, not an autodownload (per pinned-deps rule).

use async_trait::async_trait;
use log::{info, warn};
use piratetok_live_rs::structs::proto::messages::WebcastChatMessage;
use piratetok_live_rs::structs::TikTokLiveEvent;
use piratetok_live_rs::TikTokLive;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::sync::{mpsc, Mutex as TokioMutex};
use tokio::task::JoinHandle;

/// Cap on the upstream protobuf handshake. Without this, a TikTok
/// protocol rotation (the documented stability caveat at the top of
/// this file) can leave `TikTokLive::builder(...).connect()` hanging
/// indefinitely — operator sees `Connecting` forever instead of a
/// fail-loud error pointing at the maintainer-ritual dep bump.
const TIKTOK_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

use crate::models::{
    ChatConnectionStatus, ChatCredentials, ChatMessage, ChatPlatform as ChatPlatformEnum,
    MessageFlags,
};

use super::platform::{ChatPlatform, PlatformError, PlatformResult};
use super::self_echo::{SelfClass, SelfEcho};

const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTING: u8 = 1;
const STATUS_CONNECTED: u8 = 2;
const STATUS_ERROR: u8 = 3;

fn status_to_u8(status: ChatConnectionStatus) -> u8 {
    match status {
        ChatConnectionStatus::Disconnected => STATUS_DISCONNECTED,
        ChatConnectionStatus::Connecting => STATUS_CONNECTING,
        ChatConnectionStatus::Connected => STATUS_CONNECTED,
        ChatConnectionStatus::Error => STATUS_ERROR,
    }
}

fn status_from_u8(value: u8) -> ChatConnectionStatus {
    match value {
        STATUS_CONNECTING => ChatConnectionStatus::Connecting,
        STATUS_CONNECTED => ChatConnectionStatus::Connected,
        STATUS_ERROR => ChatConnectionStatus::Error,
        _ => ChatConnectionStatus::Disconnected,
    }
}

/// Builds a [`ChatMessage`] from an upstream TikTok chat event, or `None` when
/// the comment is blank. Pure — the event loop owns delivery + counting.
pub(super) fn parse_tiktok_chat(msg: &WebcastChatMessage) -> Option<ChatMessage> {
    let content = msg.comment.trim().to_string();
    if content.is_empty() {
        return None;
    }
    let nick = msg
        .user
        .as_ref()
        .map(|u| u.nickname.clone())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "Unknown".to_string());

    let mut chat_msg = ChatMessage::new(ChatPlatformEnum::TikTok, nick, content);
    // TikTok's `Common.msg_id` dedupes if the WS replays a frame; route it
    // through `with_source_id` to mint a stable cross-event id. Falls back to
    // the ChatMessage::new UUID when absent (msg_id == 0).
    if let Some(msg_id) = msg.common.as_ref().map(|c| c.msg_id).filter(|&id| id != 0) {
        chat_msg = chat_msg.with_source_id(msg_id.to_string());
    }

    Some(chat_msg)
}

pub struct TikTokConnector {
    status: Arc<AtomicU8>,
    last_error: Arc<StdMutex<Option<String>>>,
    message_count: Arc<AtomicU64>,
    disconnecting: Arc<AtomicBool>,
    disconnect_tx: Option<mpsc::Sender<()>>,
    /// Q3: handle to the upstream `next_event()` poll task. Pre-this
    /// fix the spawn handle was dropped; a connector dropped without
    /// `disconnect()` (panic-disconnect, test teardown, or even a
    /// protocol-rotation-triggered fast disconnect) leaked the task.
    /// `Drop` aborts so the runtime reclaims it.
    task_handle: Arc<TokioMutex<Option<JoinHandle<()>>>>,
}

impl Drop for TikTokConnector {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.task_handle.try_lock() {
            if let Some(handle) = guard.take() {
                handle.abort();
            }
        }
    }
}

impl TikTokConnector {
    pub fn new() -> Self {
        Self {
            status: Arc::new(AtomicU8::new(STATUS_DISCONNECTED)),
            last_error: Arc::new(StdMutex::new(None)),
            message_count: Arc::new(AtomicU64::new(0)),
            disconnecting: Arc::new(AtomicBool::new(false)),
            disconnect_tx: None,
            task_handle: Arc::new(TokioMutex::new(None)),
        }
    }

    fn set_error(&self, message: impl Into<String>) {
        self.status
            .store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = Some(message.into());
        }
    }
}

#[async_trait]
impl ChatPlatform for TikTokConnector {
    async fn connect(
        &mut self,
        credentials: ChatCredentials,
        message_tx: mpsc::Sender<ChatMessage>,
    ) -> PlatformResult<()> {
        if self.is_connected() {
            return Err(PlatformError::AlreadyConnected);
        }

        self.status.store(
            status_to_u8(ChatConnectionStatus::Connecting),
            Ordering::Relaxed,
        );
        self.disconnecting.store(false, Ordering::Relaxed);
        self.message_count.store(0, Ordering::Relaxed);
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = None;
        }

        let (username, self_identity) = match credentials {
            ChatCredentials::TikTok {
                username,
                self_identity,
                ..
            } => (username, self_identity),
            _ => {
                self.set_error("Expected TikTok credentials");
                return Err(PlatformError::InvalidConfig(
                    "Expected TikTok credentials".to_string(),
                ));
            }
        };

        let username = username.trim().trim_start_matches('@').to_string();
        if username.is_empty() {
            self.set_error("TikTok username is required");
            return Err(PlatformError::InvalidConfig(
                "TikTok username is required".to_string(),
            ));
        }

        // Cap the upstream handshake — see TIKTOK_CONNECT_TIMEOUT.
        let mut stream = tokio::time::timeout(
            TIKTOK_CONNECT_TIMEOUT,
            TikTokLive::builder(&username).connect(),
        )
        .await
        .map_err(|_| {
            let msg = format!(
                "TikTok Live handshake timed out after {}s — TikTok may have rotated \
                 the protobuf protocol; bump piratetok-live-rs",
                TIKTOK_CONNECT_TIMEOUT.as_secs()
            );
            self.set_error(msg.clone());
            PlatformError::Connection(msg)
        })?
        .map_err(|e| {
            let msg = format!("TikTok Live connection failed: {e}");
            self.set_error(msg.clone());
            PlatformError::Connection(msg)
        })?;

        let (disconnect_tx, mut disconnect_rx) = mpsc::channel::<()>(1);
        self.disconnect_tx = Some(disconnect_tx);
        self.status.store(
            status_to_u8(ChatConnectionStatus::Connected),
            Ordering::Relaxed,
        );

        // TikTok is read-only (no app send → no echoes), so this only marks the
        // local user's own natively-typed messages as "you". Nicknames are
        // display names — matched case-insensitively; `None` ⇒ no self-marking.
        let self_echo = Arc::new(SelfEcho::new(self_identity, true));

        let status = self.status.clone();
        let last_error = self.last_error.clone();
        let message_count = self.message_count.clone();
        let disconnecting = self.disconnecting.clone();

        let task_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = disconnect_rx.recv() => {
                        info!("TikTok chat disconnect requested");
                        break;
                    }
                    next = stream.next_event() => {
                        match next {
                            Some(TikTokLiveEvent::Chat(msg)) => {
                                if let Some(mut chat_msg) = parse_tiktok_chat(&msg) {
                                    // Mark the local user's own message as "you".
                                    if self_echo.classify(&chat_msg.username, &chat_msg.message)
                                        == SelfClass::Native
                                    {
                                        chat_msg.flags |= MessageFlags::SELF_AUTHOR;
                                    }
                                    if message_tx.send(chat_msg).await.is_err() {
                                        warn!("TikTok chat receiver dropped; stopping task");
                                        break;
                                    }
                                    message_count.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                            Some(TikTokLiveEvent::Disconnected) => {
                                info!("TikTok Live stream ended (host went offline or disconnected)");
                                break;
                            }
                            Some(_) => {
                                // Gifts / likes / joins / 60+ other event
                                // types — not surfaced as chat messages.
                                // Future: route gifts through a separate
                                // event-bus channel if the UI wants them.
                            }
                            None => {
                                warn!("TikTok event stream returned None; reconnect required");
                                break;
                            }
                        }
                    }
                }
            }

            if !disconnecting.load(Ordering::Relaxed) {
                status.store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
                if let Ok(mut guard) = last_error.lock() {
                    if guard.is_none() {
                        *guard = Some(
                            "TikTok connection lost — host may have gone offline, \
                             or TikTok rotated the protobuf protocol"
                                .to_string(),
                        );
                    }
                }
            }
            info!("TikTok chat task stopped");
        });

        // Q3: capture the upstream-poll task handle so `Drop` aborts it.
        {
            let mut guard = self.task_handle.lock().await;
            if let Some(prev) = guard.take() {
                prev.abort();
            }
            *guard = Some(task_handle);
        }

        info!("Connected to TikTok Live chat for @{username}");
        Ok(())
    }

    async fn disconnect(&mut self) -> PlatformResult<()> {
        if !self.is_connected() {
            return Err(PlatformError::NotConnected);
        }

        self.disconnecting.store(true, Ordering::Relaxed);
        if let Some(tx) = self.disconnect_tx.take() {
            let _ = tx.send(()).await;
        }

        self.status.store(
            status_to_u8(ChatConnectionStatus::Disconnected),
            Ordering::Relaxed,
        );
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = None;
        }

        Ok(())
    }

    fn status(&self) -> ChatConnectionStatus {
        status_from_u8(self.status.load(Ordering::Relaxed))
    }

    fn message_count(&self) -> u64 {
        self.message_count.load(Ordering::Relaxed)
    }

    fn platform_name(&self) -> &'static str {
        "tiktok"
    }

    fn can_send(&self) -> bool {
        false
    }

    async fn send_message(&mut self, _message: String) -> PlatformResult<()> {
        Err(PlatformError::Platform(
            "TikTok chat is read-only via SpiritStream — TikTok rejects \
             third-party chat-send for non-creator-app integrations"
                .to_string(),
        ))
    }

    fn last_error(&self) -> Option<String> {
        self.last_error.lock().ok().and_then(|e| e.clone())
    }
}

impl Default for TikTokConnector {
    fn default() -> Self {
        Self::new()
    }
}
