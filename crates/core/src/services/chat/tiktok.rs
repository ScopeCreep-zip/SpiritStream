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
use piratetok_live_rs::structs::TikTokLiveEvent;
use piratetok_live_rs::TikTokLive;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::mpsc;

use crate::models::{
    ChatConnectionStatus, ChatCredentials, ChatMessage, ChatPlatform as ChatPlatformEnum,
};

use super::platform::{ChatPlatform, PlatformError, PlatformResult};

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

pub struct TikTokConnector {
    status: Arc<AtomicU8>,
    last_error: Arc<StdMutex<Option<String>>>,
    message_count: Arc<AtomicU64>,
    disconnecting: Arc<AtomicBool>,
    disconnect_tx: Option<mpsc::Sender<()>>,
}

impl TikTokConnector {
    pub fn new() -> Self {
        Self {
            status: Arc::new(AtomicU8::new(STATUS_DISCONNECTED)),
            last_error: Arc::new(StdMutex::new(None)),
            message_count: Arc::new(AtomicU64::new(0)),
            disconnecting: Arc::new(AtomicBool::new(false)),
            disconnect_tx: None,
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
        message_tx: mpsc::UnboundedSender<ChatMessage>,
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

        let username = match credentials {
            ChatCredentials::TikTok { username, .. } => username,
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

        let mut stream = TikTokLive::builder(&username).connect().await.map_err(|e| {
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

        let status = self.status.clone();
        let last_error = self.last_error.clone();
        let message_count = self.message_count.clone();
        let disconnecting = self.disconnecting.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = disconnect_rx.recv() => {
                        info!("TikTok chat disconnect requested");
                        break;
                    }
                    next = stream.next_event() => {
                        match next {
                            Some(TikTokLiveEvent::Chat(msg)) => {
                                let nick = msg
                                    .user
                                    .as_ref()
                                    .map(|u| u.nickname.clone())
                                    .filter(|n| !n.is_empty())
                                    .unwrap_or_else(|| "Unknown".to_string());
                                let content = msg.comment.trim().to_string();
                                if content.is_empty() {
                                    continue;
                                }

                                let mut chat_msg = ChatMessage::new(
                                    ChatPlatformEnum::TikTok,
                                    nick,
                                    content,
                                );
                                // TikTok's `Common.msg_id` dedupes if the
                                // WS replays a frame; route it through
                                // `with_source_id` to mint a stable
                                // cross-event id. Falls back to the
                                // ChatMessage::new UUID when absent.
                                if let Some(msg_id) = msg
                                    .common
                                    .as_ref()
                                    .map(|c| c.msg_id)
                                    .filter(|&id| id != 0)
                                {
                                    chat_msg = chat_msg.with_source_id(msg_id.to_string());
                                }

                                if message_tx.send(chat_msg).is_err() {
                                    warn!("TikTok chat receiver dropped; stopping task");
                                    break;
                                }
                                message_count.fetch_add(1, Ordering::Relaxed);
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
