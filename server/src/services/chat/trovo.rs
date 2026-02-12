use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use std::env;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

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

async fn fetch_chat_token(client_id: &str, channel_id: &str) -> Result<String, PlatformError> {
    let url = format!(
        "https://open-api.trovo.live/openplatform/chat/channel-token/{}",
        channel_id
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| PlatformError::Network(format!("Failed to build HTTP client: {e}")))?;

    let response = client
        .get(url)
        .header("Accept", "application/json")
        .header("Client-ID", client_id)
        .send()
        .await
        .map_err(|e| PlatformError::Network(format!("Failed to fetch Trovo chat token: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(PlatformError::Platform(format!(
            "Trovo token request failed ({status}): {body}"
        )));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| PlatformError::Network(format!("Failed to parse Trovo token response: {e}")))?;

    body["token"]
        .as_str()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| PlatformError::Platform("Trovo token response missing token".to_string()))
}

/// Trovo chat connector (read-only for now)
pub struct TrovoConnector {
    status: Arc<AtomicU8>,
    last_error: Arc<StdMutex<Option<String>>>,
    message_count: Arc<AtomicU64>,
    disconnecting: Arc<AtomicBool>,
    disconnect_tx: Option<mpsc::Sender<()>>,
    can_send: bool,
}

impl TrovoConnector {
    pub fn new() -> Self {
        Self {
            status: Arc::new(AtomicU8::new(STATUS_DISCONNECTED)),
            last_error: Arc::new(StdMutex::new(None)),
            message_count: Arc::new(AtomicU64::new(0)),
            disconnecting: Arc::new(AtomicBool::new(false)),
            disconnect_tx: None,
            can_send: false,
        }
    }
}

#[async_trait]
impl ChatPlatform for TrovoConnector {
    async fn connect(
        &mut self,
        credentials: ChatCredentials,
        message_tx: mpsc::UnboundedSender<ChatMessage>,
    ) -> PlatformResult<()> {
        if self.is_connected() {
            return Err(PlatformError::AlreadyConnected);
        }

        self.status
            .store(status_to_u8(ChatConnectionStatus::Connecting), Ordering::Relaxed);
        self.disconnecting.store(false, Ordering::Relaxed);
        self.message_count.store(0, Ordering::Relaxed);
        self.can_send = false;
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = None;
        }

        let channel_id = match credentials {
            ChatCredentials::Trovo { channel_id } => channel_id,
            _ => {
                self.status
                    .store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
                if let Ok(mut guard) = self.last_error.lock() {
                    *guard = Some("Expected Trovo credentials".to_string());
                }
                return Err(PlatformError::InvalidConfig(
                    "Expected Trovo credentials".to_string(),
                ));
            }
        };

        let client_id = env::var("SPIRITSTREAM_TROVO_CLIENT_ID")
            .or_else(|_| env::var("TROVO_CLIENT_ID"))
            .map_err(|_| {
            self.status
                .store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
            if let Ok(mut guard) = self.last_error.lock() {
                *guard = Some(
                    "Missing SPIRITSTREAM_TROVO_CLIENT_ID (or TROVO_CLIENT_ID) in environment"
                        .to_string(),
                );
            }
            PlatformError::InvalidConfig(
                "Missing SPIRITSTREAM_TROVO_CLIENT_ID (or TROVO_CLIENT_ID) in environment"
                    .to_string(),
            )
        })?;

        let token = fetch_chat_token(&client_id, &channel_id).await.map_err(|e| {
            self.status
                .store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
            if let Ok(mut guard) = self.last_error.lock() {
                *guard = Some(format!("{e}"));
            }
            e
        })?;

        let (ws_stream, _) = connect_async("wss://open-chat.trovo.live/chat")
            .await
            .map_err(|e| {
                self.status
                    .store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
                if let Ok(mut guard) = self.last_error.lock() {
                    *guard = Some(format!("Trovo websocket connection failed: {e}"));
                }
                PlatformError::Connection(format!("Trovo websocket connection failed: {e}"))
            })?;

        let (mut write, mut read) = ws_stream.split();
        let auth_nonce = format!("auth-{}", uuid::Uuid::new_v4());
        let auth_msg = serde_json::json!({
            "type": "AUTH",
            "nonce": auth_nonce,
            "data": { "token": token }
        });

        write
            .send(Message::Text(auth_msg.to_string()))
            .await
            .map_err(|e| {
                self.status
                    .store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
                if let Ok(mut guard) = self.last_error.lock() {
                    *guard = Some(format!("Failed to send Trovo AUTH: {e}"));
                }
                PlatformError::Connection(format!("Failed to send Trovo AUTH: {e}"))
            })?;

        let auth_ok = tokio::time::timeout(Duration::from_secs(10), async {
            while let Some(frame) = read.next().await {
                let frame = frame.map_err(|e| PlatformError::Connection(format!("Trovo read error: {e}")))?;
                let text = match frame {
                    Message::Text(text) => text,
                    Message::Close(_) => {
                        return Err(PlatformError::Connection(
                            "Trovo websocket closed before AUTH completed".to_string(),
                        ));
                    }
                    _ => continue,
                };

                let payload: serde_json::Value = serde_json::from_str(&text)
                    .map_err(|e| PlatformError::Platform(format!("Invalid Trovo message: {e}")))?;

                let msg_type = payload["type"].as_str().unwrap_or_default();
                if msg_type == "RESPONSE" && payload["nonce"].as_str() == Some(auth_nonce.as_str()) {
                    if let Some(err_msg) = payload["error"].as_str() {
                        if !err_msg.is_empty() {
                            return Err(PlatformError::Authentication(format!(
                                "Trovo AUTH failed: {err_msg}"
                            )));
                        }
                    }
                    return Ok(());
                }
            }
            Err(PlatformError::Connection(
                "Trovo websocket ended before AUTH response".to_string(),
            ))
        })
        .await
        .map_err(|_| PlatformError::Connection("Timed out waiting for Trovo AUTH response".to_string()))?;

        auth_ok.map_err(|e| {
            self.status
                .store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
            if let Ok(mut guard) = self.last_error.lock() {
                *guard = Some(format!("{e}"));
            }
            e
        })?;

        let (disconnect_tx, mut disconnect_rx) = mpsc::channel::<()>(1);
        self.disconnect_tx = Some(disconnect_tx);
        self.status
            .store(status_to_u8(ChatConnectionStatus::Connected), Ordering::Relaxed);

        let status = self.status.clone();
        let last_error = self.last_error.clone();
        let message_count = self.message_count.clone();
        let disconnecting = self.disconnecting.clone();

        tokio::spawn(async move {
            let mut heartbeat = tokio::time::interval(Duration::from_secs(30));

            loop {
                tokio::select! {
                    _ = heartbeat.tick() => {
                        let ping = serde_json::json!({
                            "type": "PING",
                            "nonce": format!("ping-{}", uuid::Uuid::new_v4()),
                        });
                        if let Err(err) = write.send(Message::Text(ping.to_string())).await {
                            error!("Trovo heartbeat send failed: {}", err);
                            if let Ok(mut guard) = last_error.lock() {
                                *guard = Some("Trovo heartbeat failed".to_string());
                            }
                            status.store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
                            break;
                        }
                    }
                    _ = disconnect_rx.recv() => {
                        let _ = write.send(Message::Close(None)).await;
                        break;
                    }
                    next = read.next() => {
                        let Some(frame) = next else {
                            warn!("Trovo websocket stream ended");
                            break;
                        };

                        match frame {
                            Ok(Message::Text(text)) => {
                                let payload: serde_json::Value = match serde_json::from_str(&text) {
                                    Ok(v) => v,
                                    Err(err) => {
                                        warn!("Failed to parse Trovo message: {}", err);
                                        continue;
                                    }
                                };

                                let msg_type = payload["type"].as_str().unwrap_or_default();
                                if msg_type != "CHAT" {
                                    continue;
                                }

                                if let Some(chats) = payload["data"]["chats"].as_array() {
                                    let mut emitted = 0_u64;
                                    for chat in chats {
                                        let content = chat["content"].as_str().unwrap_or_default().trim().to_string();
                                        if content.is_empty() {
                                            continue;
                                        }
                                        let username = chat["nick_name"]
                                            .as_str()
                                            .or_else(|| chat["user_name"].as_str())
                                            .unwrap_or("Unknown")
                                            .to_string();

                                        let mut msg = ChatMessage::new(
                                            ChatPlatformEnum::Trovo,
                                            username,
                                            content,
                                        );

                                        if let Some(message_id) = chat["message_id"].as_str() {
                                            msg = msg.with_source_id(message_id.to_string());
                                        }
                                        if let Some(send_time) = chat["send_time"].as_i64() {
                                            msg.timestamp = if send_time > 1_000_000_000_000 {
                                                send_time
                                            } else {
                                                send_time * 1000
                                            };
                                        }
                                        if let Some(roles) = chat["roles"].as_array() {
                                            let badges: Vec<String> = roles
                                                .iter()
                                                .filter_map(|r| r.as_str().map(|s| s.to_string()))
                                                .collect();
                                            if !badges.is_empty() {
                                                msg = msg.with_badges(badges);
                                            }
                                        }

                                        if message_tx.send(msg).is_err() {
                                            warn!("Failed to deliver Trovo chat message: receiver dropped");
                                            break;
                                        }
                                        emitted += 1;
                                    }
                                    if emitted > 0 {
                                        message_count.fetch_add(emitted, Ordering::Relaxed);
                                    }
                                }
                            }
                            Ok(Message::Ping(data)) => {
                                if let Err(err) = write.send(Message::Pong(data)).await {
                                    warn!("Failed to reply to Trovo ping: {}", err);
                                }
                            }
                            Ok(Message::Close(_)) => break,
                            Ok(_) => {}
                            Err(err) => {
                                warn!("Trovo websocket read error: {}", err);
                                if let Ok(mut guard) = last_error.lock() {
                                    *guard = Some(format!("Trovo read error: {err}"));
                                }
                                status.store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
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
                        *guard = Some("Trovo connection lost".to_string());
                    }
                }
            }
            info!("Trovo chat task stopped");
        });

        info!("Connected to Trovo chat for channel ID {}", channel_id);
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

        self.status
            .store(status_to_u8(ChatConnectionStatus::Disconnected), Ordering::Relaxed);
        self.can_send = false;
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
        "trovo"
    }

    fn can_send(&self) -> bool {
        self.can_send && self.is_connected()
    }

    fn last_error(&self) -> Option<String> {
        self.last_error.lock().ok().and_then(|e| e.clone())
    }
}

impl Default for TrovoConnector {
    fn default() -> Self {
        Self::new()
    }
}
