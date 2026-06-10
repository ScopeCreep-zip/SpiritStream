//! Kick.com chat connector.
//!
//! Kick uses Pusher Channels over WebSocket for chat events (their
//! frontend is a Pusher-protocol client). The `pusher:subscribe`
//! channel for a given chatroom is `chatrooms.{chatroom_id}.v2` and the
//! interesting event is `App\Events\ChatMessageEvent` whose `data`
//! field is a JSON-string payload with the message body.
//!
//! Read path requires no authentication — the Pusher subscription is
//! open for public chatrooms. Send path posts to Kick's official
//! public REST API at `api.kick.com/public/v1/chat`, which requires
//! an OAuth 2.1 bearer with `chat:write` scope.
//!
//! Chatroom id lookup: Kick's public REST exposes a `channels` lookup
//! at `/api/v2/channels/{slug}` that returns the chatroom id we need
//! for the subscription channel.

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::sync::{mpsc, Mutex as TokioMutex};
use tokio::task::JoinHandle;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::models::{
    ChatConnectionStatus, ChatCredentials, ChatMessage, ChatPlatform as ChatPlatformEnum,
};

use super::endpoints::ChatEndpoints;
use super::platform::{ChatPlatform, PlatformError, PlatformResult};

const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTING: u8 = 1;
const STATUS_CONNECTED: u8 = 2;
const STATUS_ERROR: u8 = 3;

/// Pusher ping interval. The Pusher protocol expects pings at least
/// every 120s; 60s gives us a safety margin.
const PING_INTERVAL: Duration = Duration::from_secs(60);
/// Cap on the WebSocket handshake itself. Without this `connect_async`
/// hangs indefinitely if Pusher accepts the TCP connection but never
/// completes the upgrade — operator sees `Connecting` forever.
const WS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// Cap on the `pusher_internal:subscription_succeeded` ACK after we
/// send the `pusher:subscribe` frame. Without this, a rejected chatroom
/// (deleted, banned, anti-bot blocked) leaves the connector reporting
/// `Connected` with zero messages flowing — silent dead-stream.
const SUBSCRIBE_ACK_TIMEOUT: Duration = Duration::from_secs(5);

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

/// Fetch the chatroom id for a Kick channel slug. Anonymous public
/// endpoint — no auth needed.
async fn fetch_chatroom_id(channel_slug: &str, channel_lookup: &str) -> Result<u64, PlatformError> {
    let url = format!("{channel_lookup}{}", urlencoding::encode(channel_slug));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        // Kick's Cloudflare WAF blocks the default reqwest user-agent.
        .user_agent("Mozilla/5.0 SpiritStream chat-connector")
        .build()
        .map_err(|e| PlatformError::Network(format!("Failed to build HTTP client: {e}")))?;

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| PlatformError::Network(format!("Kick channel lookup failed: {e}")))?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(PlatformError::InvalidConfig(format!(
            "Kick channel '{channel_slug}' not found"
        )));
    }
    if !response.status().is_success() {
        let status = response.status();
        return Err(PlatformError::Platform(format!(
            "Kick channel lookup failed ({status})"
        )));
    }

    let body: serde_json::Value = response.json().await.map_err(|e| {
        PlatformError::Network(format!("Failed to parse Kick channel response: {e}"))
    })?;

    body["chatroom"]["id"]
        .as_u64()
        .ok_or_else(|| PlatformError::Platform("Kick response missing chatroom.id".to_string()))
}

/// Parse a Kick Pusher frame into a chat message.
///
/// Returns `None` for any frame that is not an `App\Events\ChatMessageEvent`,
/// whose JSON-string `data` body fails to parse, or whose content is blank.
/// Pusher wraps the application payload in a JSON-string `data` field, so the
/// body is parsed a second time here. Pure — the websocket loop owns delivery,
/// counting, and the outer-frame JSON parse + its warning.
pub(super) fn parse_kick_chat_event(payload: &serde_json::Value) -> Option<ChatMessage> {
    if payload["event"].as_str().unwrap_or("") != "App\\Events\\ChatMessageEvent" {
        // Pusher control frames (subscription_succeeded, pong) + other
        // Kick events (raid, follower) are ignored for now.
        return None;
    }

    let data_str = payload["data"].as_str().unwrap_or("");
    if data_str.is_empty() {
        return None;
    }
    let chat: serde_json::Value = serde_json::from_str(data_str).ok()?;

    let content = chat["content"].as_str().unwrap_or("").trim().to_string();
    if content.is_empty() {
        return None;
    }
    let username = chat["sender"]["username"]
        .as_str()
        .unwrap_or("Unknown")
        .to_string();

    let mut msg = ChatMessage::new(ChatPlatformEnum::Kick, username, content);

    if let Some(id) = chat["id"].as_str() {
        msg = msg.with_source_id(id.to_string());
    }
    if let Some(created_at) = chat["created_at"].as_str() {
        if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(created_at) {
            msg.timestamp = parsed.timestamp_millis();
        }
    }
    if let Some(color) = chat["sender"]["identity"]["color"].as_str() {
        msg = msg.with_color(color.to_string());
    }
    if let Some(badges) = chat["sender"]["identity"]["badges"].as_array() {
        let badge_names: Vec<String> = badges
            .iter()
            .filter_map(|b| b["type"].as_str().map(|s| s.to_string()))
            .collect();
        if !badge_names.is_empty() {
            msg = msg.with_badges(badge_names);
        }
    }

    Some(msg)
}

/// Kick chat connector.
///
/// Reads chat anonymously via Pusher WebSocket. Sending requires an
/// OAuth bearer with `chat:write` plus the broadcaster user id (the
/// numeric id Kick assigns to the user — distinct from the slug).
pub struct KickConnector {
    status: Arc<AtomicU8>,
    last_error: Arc<StdMutex<Option<String>>>,
    message_count: Arc<AtomicU64>,
    disconnecting: Arc<AtomicBool>,
    disconnect_tx: Option<mpsc::Sender<()>>,
    /// Send credentials captured at `connect()` time. `None` keeps the
    /// connector in read-only mode.
    send_state: Arc<StdMutex<Option<SendState>>>,
    /// Q1: handle to the background WS poll task spawned in `connect()`.
    /// Pre-Q1 the spawn handle was dropped on the floor, so a connector
    /// dropped without `disconnect()` (panic-disconnect path, test
    /// teardown, profile reactivation) leaked the task. `Drop` aborts
    /// the handle so the runtime reclaims the WS frame loop.
    task_handle: Arc<TokioMutex<Option<JoinHandle<()>>>>,
    /// Kick Pusher Channels WebSocket URL (injected from `ChatEndpoints`).
    pusher_ws: String,
    /// Kick public REST channel-lookup prefix; the slug is appended.
    channel_lookup: String,
    /// Kick official REST chat-send endpoint.
    send_chat: String,
}

impl Drop for KickConnector {
    fn drop(&mut self) {
        // Best-effort cancellation — `Drop` isn't async. `try_lock`
        // avoids blocking on lock contention from a dying connector.
        if let Ok(mut guard) = self.task_handle.try_lock() {
            if let Some(handle) = guard.take() {
                handle.abort();
            }
        }
    }
}

#[derive(Clone)]
struct SendState {
    oauth_token: String,
    broadcaster_user_id: u64,
}

impl KickConnector {
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
            send_state: Arc::new(StdMutex::new(None)),
            task_handle: Arc::new(TokioMutex::new(None)),
            pusher_ws: endpoints.kick_pusher_ws.clone(),
            channel_lookup: endpoints.kick_channel_lookup.clone(),
            send_chat: endpoints.kick_send_chat.clone(),
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
impl ChatPlatform for KickConnector {
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

        let (channel, oauth_token, broadcaster_user_id) = match credentials {
            ChatCredentials::Kick {
                channel,
                oauth_token,
                broadcaster_user_id,
            } => (channel, oauth_token, broadcaster_user_id),
            _ => {
                self.set_error("Expected Kick credentials");
                return Err(PlatformError::InvalidConfig(
                    "Expected Kick credentials".to_string(),
                ));
            }
        };

        let channel = channel.trim().to_lowercase();
        if channel.is_empty() {
            self.set_error("Kick channel name is required");
            return Err(PlatformError::InvalidConfig(
                "Kick channel name is required".to_string(),
            ));
        }

        let chatroom_id = fetch_chatroom_id(&channel, &self.channel_lookup)
            .await
            .inspect_err(|e| self.set_error(e.to_string()))?;

        // Cap the WS handshake itself — `connect_async` has no built-in
        // timeout and will hang forever if Pusher accepts TCP but never
        // completes the upgrade.
        let (ws_stream, _) =
            tokio::time::timeout(WS_HANDSHAKE_TIMEOUT, connect_async(self.pusher_ws.as_str()))
                .await
                .map_err(|_| {
                    self.set_error("Kick websocket handshake timed out");
                    PlatformError::Connection("Kick websocket handshake timed out".to_string())
                })?
                .map_err(|e| {
                    self.set_error(format!("Kick websocket connection failed: {e}"));
                    PlatformError::Connection(format!("Kick websocket connection failed: {e}"))
                })?;

        let (mut write, mut read) = ws_stream.split();

        // The Pusher protocol's first frame is `pusher:connection_established`.
        // Wait for it before we subscribe; otherwise the server may drop our
        // subscribe as out-of-band.
        let established = tokio::time::timeout(Duration::from_secs(10), async {
            while let Some(frame) = read.next().await {
                let frame = frame
                    .map_err(|e| PlatformError::Connection(format!("Pusher read error: {e}")))?;
                if let Message::Text(text) = frame {
                    let payload: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
                        PlatformError::Platform(format!("Invalid Pusher frame: {e}"))
                    })?;
                    if payload["event"].as_str() == Some("pusher:connection_established") {
                        return Ok(());
                    }
                }
            }
            Err(PlatformError::Connection(
                "Pusher closed before connection_established".to_string(),
            ))
        })
        .await
        .map_err(|_| {
            PlatformError::Connection(
                "Timed out waiting for Pusher connection_established".to_string(),
            )
        })?;

        established.inspect_err(|e| self.set_error(e.to_string()))?;

        let channel_name = format!("chatrooms.{chatroom_id}.v2");
        let subscribe_payload = serde_json::json!({
            "event": "pusher:subscribe",
            "data": { "channel": channel_name }
        });
        write
            .send(Message::Text(subscribe_payload.to_string()))
            .await
            .map_err(|e| {
                self.set_error(format!("Failed to subscribe to Kick chatroom: {e}"));
                PlatformError::Connection(format!("Failed to subscribe to Kick chatroom: {e}"))
            })?;

        // Wait for the subscription ACK before reporting Connected.
        // Pre-this fix a rejected chatroom (deleted, banned, anti-bot
        // blocked) left the connector reporting Connected with zero
        // messages flowing. Pusher emits one of:
        //   `pusher_internal:subscription_succeeded`  (channel == ours)
        //   `pusher:subscription_error`               (data carries reason)
        // Fail loud on either timeout or explicit error so the operator
        // sees the failure instead of a silent dead chat surface.
        let subscribe_ack = tokio::time::timeout(SUBSCRIBE_ACK_TIMEOUT, async {
            while let Some(frame) = read.next().await {
                let frame = frame
                    .map_err(|e| PlatformError::Connection(format!("Pusher read error: {e}")))?;
                let Message::Text(text) = frame else {
                    continue;
                };
                let payload: serde_json::Value = serde_json::from_str(&text)
                    .map_err(|e| PlatformError::Platform(format!("Invalid Pusher frame: {e}")))?;
                let event = payload["event"].as_str().unwrap_or("");
                let chan = payload["channel"].as_str().unwrap_or("");
                if event == "pusher_internal:subscription_succeeded" && chan == channel_name {
                    return Ok(());
                }
                if event == "pusher:subscription_error" {
                    let detail = payload["data"].as_str().unwrap_or("unknown");
                    return Err(PlatformError::Connection(format!(
                        "Kick rejected subscribe to {chan}: {detail}"
                    )));
                }
            }
            Err(PlatformError::Connection(
                "Pusher closed before subscription_succeeded".to_string(),
            ))
        })
        .await
        .map_err(|_| {
            PlatformError::Connection(format!(
                "Kick chatroom subscribe ACK timed out (chatroom={chatroom_id})"
            ))
        })?;
        subscribe_ack.inspect_err(|e| self.set_error(e.to_string()))?;

        // Cache send credentials (if any) for outbound messages.
        let captured_send_state =
            if let (Some(token), Some(broadcaster)) = (oauth_token.clone(), broadcaster_user_id) {
                Some(SendState {
                    oauth_token: token,
                    broadcaster_user_id: broadcaster,
                })
            } else {
                None
            };
        if let Ok(mut guard) = self.send_state.lock() {
            *guard = captured_send_state;
        }

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

        let task_handle = tokio::spawn(async move {
            let mut ping = tokio::time::interval(PING_INTERVAL);
            // First tick fires immediately; skip to align with PING_INTERVAL.
            ping.tick().await;

            loop {
                tokio::select! {
                    _ = ping.tick() => {
                        let ping_payload = serde_json::json!({
                            "event": "pusher:ping",
                            "data": "{}",
                        });
                        if let Err(err) = write.send(Message::Text(ping_payload.to_string())).await {
                            error!("Kick pusher ping send failed: {}", err);
                            if let Ok(mut guard) = last_error.lock() {
                                *guard = Some(format!("Kick heartbeat failed: {err}"));
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
                            warn!("Kick pusher stream ended");
                            break;
                        };

                        match frame {
                            Ok(Message::Text(text)) => {
                                let payload: serde_json::Value = match serde_json::from_str(&text) {
                                    Ok(v) => v,
                                    Err(err) => {
                                        warn!("Failed to parse Kick pusher frame: {}", err);
                                        continue;
                                    }
                                };

                                if let Some(msg) = parse_kick_chat_event(&payload) {
                                    if message_tx.send(msg).await.is_err() {
                                        warn!("Failed to deliver Kick chat message: receiver dropped");
                                        break;
                                    }
                                    message_count.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                            Ok(Message::Ping(data)) => {
                                if let Err(err) = write.send(Message::Pong(data)).await {
                                    warn!("Failed to reply to Kick ping: {}", err);
                                }
                            }
                            Ok(Message::Close(_)) => break,
                            Ok(_) => {}
                            Err(err) => {
                                warn!("Kick pusher read error: {}", err);
                                if let Ok(mut guard) = last_error.lock() {
                                    *guard = Some(format!("Kick read error: {err}"));
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
                        *guard = Some("Kick connection lost".to_string());
                    }
                }
            }
            info!("Kick chat task stopped");
        });

        // Q1: capture the task handle so `Drop` can abort it. Take the
        // lock async because `connect()` is already async; this is the
        // only contended path so cost is trivial.
        {
            let mut guard = self.task_handle.lock().await;
            // Abort any prior task too (defensive — `is_connected()`
            // check above should have rejected reconnect, but if state
            // ever desyncs we still want a clean replacement).
            if let Some(prev) = guard.take() {
                prev.abort();
            }
            *guard = Some(task_handle);
        }

        info!("Connected to Kick chat for channel '{channel}' (chatroom_id={chatroom_id})");
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
        if let Ok(mut guard) = self.send_state.lock() {
            *guard = None;
        }
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
        "kick"
    }

    fn can_send(&self) -> bool {
        if !self.is_connected() {
            return false;
        }
        self.send_state.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    async fn send_message(&mut self, message: String) -> PlatformResult<()> {
        if !self.is_connected() {
            return Err(PlatformError::NotConnected);
        }
        let state = match self.send_state.lock() {
            Ok(g) => g.clone(),
            Err(_) => None,
        };
        let state = state.ok_or_else(|| {
            PlatformError::Platform(
                "Kick send disabled — no OAuth token + broadcaster id captured at connect"
                    .to_string(),
            )
        })?;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| {
                PlatformError::Network(format!("Failed to build Kick HTTP client: {e}"))
            })?;

        let body = serde_json::json!({
            "content": message,
            "type": "user",
            "broadcaster_user_id": state.broadcaster_user_id,
        });

        let response = client
            .post(self.send_chat.as_str())
            .bearer_auth(&state.oauth_token)
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| PlatformError::Network(format!("Kick send request failed: {e}")))?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(PlatformError::Authentication(
                "Kick rejected the OAuth token — re-auth required".to_string(),
            ));
        }
        if !status.is_success() {
            // Same fail-loud rule as the YouTube H6 fix — pre-this fix
            // a body-read failure produced `Kick send failed (500): `
            // with empty diagnostic, hiding rate-limit / quota /
            // anti-bot rejections behind a status-only error.
            let detail = match response.text().await {
                Ok(b) => b,
                Err(e) => format!("<body read failed: {e}>"),
            };
            return Err(PlatformError::Platform(format!(
                "Kick send failed ({status}): {}",
                detail.chars().take(200).collect::<String>()
            )));
        }
        Ok(())
    }

    fn last_error(&self) -> Option<String> {
        self.last_error.lock().ok().and_then(|e| e.clone())
    }
}

impl Default for KickConnector {
    fn default() -> Self {
        Self::new()
    }
}
