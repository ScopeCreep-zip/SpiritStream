use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::models::{
    ChatConnectionStatus, ChatCredentials, ChatMessage, ChatPlatform as ChatPlatformEnum,
    MessageFlags,
};

use super::endpoints::ChatEndpoints;
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

/// Parse a Trovo `CHAT` frame body into chat messages.
///
/// Returns an empty vec for any non-`CHAT` frame or a `CHAT` frame whose
/// `data.chats` is absent/empty, and skips individual entries with blank
/// content. Pure — the websocket loop owns delivery and message counting,
/// and still logs + skips frames that fail the outer JSON parse.
pub(super) fn parse_trovo_chats(payload: &serde_json::Value) -> Vec<ChatMessage> {
    if payload["type"].as_str().unwrap_or_default() != "CHAT" {
        return Vec::new();
    }
    let Some(chats) = payload["data"]["chats"].as_array() else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for chat in chats {
        let content = chat["content"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_string();
        if content.is_empty() {
            continue;
        }
        let username = chat["nick_name"]
            .as_str()
            .or_else(|| chat["user_name"].as_str())
            .unwrap_or("Unknown")
            .to_string();

        let mut msg = ChatMessage::new(ChatPlatformEnum::Trovo, username, content);

        if let Some(message_id) = chat["message_id"].as_str() {
            msg = msg.with_source_id(message_id.to_string());
        }
        if let Some(send_time) = chat["send_time"].as_i64() {
            // Trovo sends seconds on some events, milliseconds on others.
            // Normalise to ms; the 10^12 threshold is ~2001 in seconds /
            // ~1970 in ms, so any plausible live timestamp lands correctly.
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

        out.push(msg);
    }
    out
}

async fn fetch_chat_token(
    client_id: &str,
    channel_id: &str,
    api_base: &str,
) -> Result<String, PlatformError> {
    let url = format!("{api_base}/openplatform/chat/channel-token/{channel_id}");

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

    let body: serde_json::Value = response.json().await.map_err(|e| {
        PlatformError::Network(format!("Failed to parse Trovo token response: {e}"))
    })?;

    body["token"]
        .as_str()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| PlatformError::Platform("Trovo token response missing token".to_string()))
}

/// Trovo chat connector. Reads via the client-id-only channel chat
/// token; SENDS via `openplatform/chat/send` when connected with an
/// OAuth token (`chat_send_self`).
pub struct TrovoConnector {
    status: Arc<AtomicU8>,
    last_error: Arc<StdMutex<Option<String>>>,
    message_count: Arc<AtomicU64>,
    disconnecting: Arc<AtomicBool>,
    disconnect_tx: Option<mpsc::Sender<()>>,
    can_send: bool,
    /// Trovo open-platform API origin (injected from `ChatEndpoints`).
    api_base: String,
    /// Trovo open-chat WebSocket URL (injected from `ChatEndpoints`).
    chat_ws: String,
    /// Captured at connect for the send path (Client-ID header).
    client_id: Option<String>,
    /// OAuth bearer for `Authorization: OAuth <token>` sends.
    oauth_token: Option<String>,
    /// Self-message detector + outbound echo dedup, built at `connect()` from
    /// the user's own Trovo username. Shared between the read task and `send`.
    self_echo: Arc<StdMutex<Option<Arc<SelfEcho>>>>,
}

impl TrovoConnector {
    pub fn new() -> Self {
        Self::with_endpoints(&ChatEndpoints::default())
    }

    pub fn with_endpoints(endpoints: &ChatEndpoints) -> Self {
        Self {
            status: Arc::new(AtomicU8::new(STATUS_DISCONNECTED)),
            last_error: Arc::new(StdMutex::new(None)),
            message_count: Arc::new(AtomicU64::new(0)),
            disconnecting: Arc::new(AtomicBool::new(false)),
            disconnect_tx: None,
            can_send: false,
            api_base: endpoints.trovo_api_base.clone(),
            chat_ws: endpoints.trovo_chat_ws.clone(),
            client_id: None,
            oauth_token: None,
            self_echo: Arc::new(StdMutex::new(None)),
        }
    }
}

#[async_trait]
impl ChatPlatform for TrovoConnector {
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
        self.can_send = false;
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = None;
        }

        let (channel_id, client_id, oauth_token, self_identity) = match credentials {
            ChatCredentials::Trovo {
                channel_id,
                client_id,
                oauth_token,
                self_identity,
            } => (channel_id, client_id, oauth_token, self_identity),
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

        // The client id arrives on the credentials, resolved by the
        // transport from the OAuth config chain (in-app setup → env →
        // embedded). Reading the environment here — the old behaviour —
        // silently ignored credentials saved through the in-app form.
        // Fail loud on absent/placeholder so the user gets an
        // actionable error, not a mystery disconnect.
        const NO_CLIENT_ID: &str =
            "Trovo client ID is not configured — complete the one-time sign-in setup in chat settings";
        let client_id = match client_id {
            Some(id) if crate::services::oauth::credential_is_real(&id) => id,
            _ => {
                self.status
                    .store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
                if let Ok(mut guard) = self.last_error.lock() {
                    *guard = Some(NO_CLIENT_ID.to_string());
                }
                return Err(PlatformError::InvalidConfig(NO_CLIENT_ID.to_string()));
            }
        };

        let token = fetch_chat_token(&client_id, &channel_id, &self.api_base)
            .await
            .map_err(|e| {
                self.status
                    .store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
                if let Ok(mut guard) = self.last_error.lock() {
                    *guard = Some(format!("{e}"));
                }
                e
            })?;

        let (ws_stream, _) = connect_async(self.chat_ws.as_str()).await.map_err(|e| {
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
                let frame = frame
                    .map_err(|e| PlatformError::Connection(format!("Trovo read error: {e}")))?;
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
                if msg_type == "RESPONSE" && payload["nonce"].as_str() == Some(auth_nonce.as_str())
                {
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
        .map_err(|_| {
            PlatformError::Connection("Timed out waiting for Trovo AUTH response".to_string())
        })?;

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
        // Send capability = an OAuth token arrived with the credentials
        // (chat_send_self). Reads work either way.
        self.client_id = Some(client_id.clone());
        self.can_send = oauth_token.is_some();
        self.oauth_token = oauth_token;
        self.status.store(
            status_to_u8(ChatConnectionStatus::Connected),
            Ordering::Relaxed,
        );

        // Trovo usernames are case-insensitive. Shared with the read task (mark
        // native self-messages) and `send_message` (record app-sent echoes).
        let self_echo = Arc::new(SelfEcho::new(self_identity, true));
        if let Ok(mut guard) = self.self_echo.lock() {
            *guard = Some(self_echo.clone());
        }

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

                                let mut emitted = 0_u64;
                                for mut msg in parse_trovo_chats(&payload) {
                                    // Drop our own app-sent echo; mark a natively
                                    // typed self-message as "you".
                                    let class = self_echo.classify(&msg.username, &msg.message);
                                    if class == SelfClass::Echo {
                                        continue;
                                    }
                                    if class == SelfClass::Native {
                                        msg.flags |= MessageFlags::SELF_AUTHOR;
                                    }
                                    if message_tx.send(msg).await.is_err() {
                                        warn!("Failed to deliver Trovo chat message: receiver dropped");
                                        break;
                                    }
                                    emitted += 1;
                                }
                                if emitted > 0 {
                                    message_count.fetch_add(emitted, Ordering::Relaxed);
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

        self.status.store(
            status_to_u8(ChatConnectionStatus::Disconnected),
            Ordering::Relaxed,
        );
        self.can_send = false;
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = None;
        }

        Ok(())
    }

    /// Send to the signed-in user's own channel via
    /// `POST {api_base}/openplatform/chat/send` (scope `chat_send_self`).
    /// Trovo's auth scheme is `Authorization: OAuth <token>` + a
    /// `Client-ID` header — not Bearer.
    async fn send_message(&mut self, message: String) -> PlatformResult<()> {
        let (Some(token), Some(client_id)) = (self.oauth_token.clone(), self.client_id.clone())
        else {
            return Err(PlatformError::Platform(
                "Trovo send requires signing in with Trovo first".to_string(),
            ));
        };
        // Record for echo-dedup: Trovo broadcasts this back over the chat socket.
        if let Some(echo) = self.self_echo.lock().ok().and_then(|g| g.clone()) {
            echo.record_outbound(&message);
        }
        let url = format!("{}/openplatform/chat/send", self.api_base);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| PlatformError::Network(format!("Failed to build HTTP client: {e}")))?;
        let response = client
            .post(url)
            .header("Accept", "application/json")
            .header("Client-ID", client_id)
            .header("Authorization", format!("OAuth {token}"))
            .json(&serde_json::json!({ "content": message }))
            .send()
            .await
            .map_err(|e| PlatformError::Network(format!("Trovo send failed: {e}")))?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            let body: String = body.chars().take(200).collect();
            return Err(PlatformError::Platform(format!(
                "Trovo send rejected ({status}): {body}"
            )));
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
