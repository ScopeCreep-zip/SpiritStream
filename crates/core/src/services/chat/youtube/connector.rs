use async_trait::async_trait;
use log::{debug, info};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch};

use crate::models::{ChatConnectionStatus, ChatCredentials, ChatMessage, YouTubeAuth};

use super::super::endpoints::ChatEndpoints;
use super::super::platform::{ChatPlatform, PlatformError, PlatformResult};

use super::auth::{find_live_chat_id, AuthMode};
use super::parse::OutboundMessage;
use super::poll::PollTask;
use super::status::{status_from_u8, status_to_u8};

/// YouTube Live Chat connector using YouTube Data API v3
pub struct YouTubeConnector {
    status: Arc<AtomicU8>,
    last_error: Arc<StdMutex<Option<String>>>,
    message_count: Arc<AtomicU64>,
    disconnecting: Arc<AtomicBool>,
    can_send: bool,
    channel_id: Option<String>,
    self_channel_id: Option<String>,
    live_chat_id: Option<String>,
    auth_mode: Option<AuthMode>,
    disconnect_tx: Option<mpsc::Sender<()>>,
    oauth_token_tx: Option<watch::Sender<String>>,
    recent_outbound: Arc<StdMutex<VecDeque<OutboundMessage>>>,
    api_base: String,
    /// Handle to THE single poll task. `is_active()` consults it so a duplicate
    /// connect is a no-op while the loop lives; `disconnect()`/`Drop` abort it.
    /// This is what makes YouTube a single-owner connection (no orphan pollers,
    /// no connect churn).
    poll_handle: Option<tokio::task::JoinHandle<()>>,
}

impl YouTubeConnector {
    pub fn new() -> Self {
        Self::with_endpoints(&ChatEndpoints::default())
    }

    pub fn with_endpoints(endpoints: &ChatEndpoints) -> Self {
        Self {
            status: Arc::new(AtomicU8::new(status_to_u8(
                ChatConnectionStatus::Disconnected,
            ))),
            last_error: Arc::new(StdMutex::new(None)),
            message_count: Arc::new(AtomicU64::new(0)),
            disconnecting: Arc::new(AtomicBool::new(false)),
            can_send: false,
            channel_id: None,
            self_channel_id: None,
            live_chat_id: None,
            auth_mode: None,
            disconnect_tx: None,
            oauth_token_tx: None,
            recent_outbound: Arc::new(StdMutex::new(VecDeque::new())),
            api_base: endpoints.youtube_api_base.clone(),
            poll_handle: None,
        }
    }
}

impl Drop for YouTubeConnector {
    fn drop(&mut self) {
        // Belt-and-suspenders against orphan pollers: if the connector is
        // dropped (e.g. replaced in the platforms map) without a clean
        // disconnect, abort its poll task so it can't keep flooding the feed.
        if let Some(handle) = self.poll_handle.take() {
            handle.abort();
        }
    }
}

#[async_trait]
impl ChatPlatform for YouTubeConnector {
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
        self.can_send = false;
        self.message_count.store(0, Ordering::Relaxed);
        if let Ok(mut recent) = self.recent_outbound.lock() {
            recent.clear();
        }
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = None;
        }

        // Extract YouTube credentials
        let (channel_id, auth_mode) = match credentials {
            ChatCredentials::YouTube { channel_id, auth } => {
                let mode = match auth {
                    YouTubeAuth::ApiKey { key } => {
                        self.oauth_token_tx = None;
                        AuthMode::ApiKey { key }
                    }
                    YouTubeAuth::AppOAuth { access_token, .. } => {
                        let (token_tx, token_rx) = watch::channel(access_token);
                        self.oauth_token_tx = Some(token_tx);
                        AuthMode::OAuth {
                            access_token_rx: token_rx,
                        }
                    }
                };
                (channel_id, mode)
            }
            _ => {
                return Err(PlatformError::InvalidConfig(
                    "Expected YouTube credentials".to_string(),
                ));
            }
        };

        info!(
            "Connecting to YouTube Live Chat for channel: {}",
            channel_id
        );
        self.status.store(
            status_to_u8(ChatConnectionStatus::Connecting),
            Ordering::Relaxed,
        );

        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| PlatformError::Network(format!("Failed to create HTTP client: {}", e)))?;

        // Find the active live chat ID + the authenticated owner channel id.
        let (live_chat_id, owner_channel_id) =
            match find_live_chat_id(&http_client, &auth_mode, &channel_id, &self.api_base).await {
                Ok(target) => {
                    info!("Found YouTube live chat ID");
                    target
                }
                Err(e) => {
                    // "Not live yet" and "quota exhausted" are waiting states,
                    // not failures: report DISCONNECTED so the UI doesn't show
                    // an error and the reconnect loop (which fires on ERROR)
                    // doesn't churn — hammering an exhausted quota every 30s
                    // only digs the hole deeper.
                    let status = if matches!(
                        e,
                        PlatformError::NotLive(_) | PlatformError::QuotaExceeded(_)
                    ) {
                        ChatConnectionStatus::Disconnected
                    } else {
                        ChatConnectionStatus::Error
                    };
                    self.status.store(status_to_u8(status), Ordering::Relaxed);
                    if let Ok(mut guard) = self.last_error.lock() {
                        *guard = Some(format!("{}", e));
                    }
                    return Err(e);
                }
            };

        self.channel_id = Some(channel_id.clone());
        self.live_chat_id = Some(live_chat_id.clone());
        self.auth_mode = Some(auth_mode.clone());
        self.can_send = matches!(auth_mode, AuthMode::OAuth { .. });
        // Self-echo suppression keys on the AUTHENTICATED owner channel id
        // (canonical `UCxxxx` from the broadcast), falling back to the typed
        // setting only if the API didn't return one. Using the typed setting
        // alone double-posts the streamer's own messages whenever it isn't the
        // exact canonical id (e.g. a handle or URL).
        self.self_channel_id = owner_channel_id.or(Some(channel_id));
        self.status.store(
            status_to_u8(ChatConnectionStatus::Connected),
            Ordering::Relaxed,
        );

        // Create disconnect channel
        let (disconnect_tx, disconnect_rx) = mpsc::channel::<()>(1);
        self.disconnect_tx = Some(disconnect_tx);

        info!("Connected to YouTube Live Chat, starting polling");

        // Spawn THE single poll task and keep its handle (drives is_active()).
        let handle = PollTask {
            status: self.status.clone(),
            last_error: self.last_error.clone(),
            message_count: self.message_count.clone(),
            disconnecting: self.disconnecting.clone(),
            recent_outbound: self.recent_outbound.clone(),
            self_channel_id: self.self_channel_id.clone(),
            api_base: self.api_base.clone(),
            auth_mode,
            live_chat_id,
            http_client,
            message_tx,
        }
        .spawn(disconnect_rx);
        self.poll_handle = Some(handle);

        Ok(())
    }

    async fn disconnect(&mut self) -> PlatformResult<()> {
        if !self.is_connected() {
            return Err(PlatformError::NotConnected);
        }

        info!("Disconnecting from YouTube Live Chat");
        self.disconnecting.store(true, Ordering::Relaxed);

        // Send the graceful disconnect signal, then abort the poll task so it
        // stops immediately (no overlap with a subsequent reconnect's poller).
        if let Some(tx) = self.disconnect_tx.take() {
            let _ = tx.send(()).await;
        }
        if let Some(handle) = self.poll_handle.take() {
            handle.abort();
        }

        self.channel_id = None;
        self.self_channel_id = None;
        self.live_chat_id = None;
        self.auth_mode = None;
        self.can_send = false;
        self.status.store(
            status_to_u8(ChatConnectionStatus::Disconnected),
            Ordering::Relaxed,
        );
        self.oauth_token_tx = None;
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = None;
        }

        info!("Disconnected from YouTube Live Chat");
        Ok(())
    }

    fn status(&self) -> ChatConnectionStatus {
        status_from_u8(self.status.load(Ordering::Relaxed))
    }

    /// "Active" = the single poll task is alive. This is the idempotency key:
    /// while a poll loop is running, a duplicate `connect()` from any of the
    /// many triggers is a no-op (vs. the default `is_connected()`, which is
    /// false during a transient Error blip and would let a 2nd poller spawn —
    /// the connect churn + duplicate flood). The task is the single owner.
    fn is_active(&self) -> bool {
        self.poll_handle
            .as_ref()
            .map(|h| !h.is_finished())
            .unwrap_or(false)
    }

    fn message_count(&self) -> u64 {
        self.message_count.load(Ordering::Relaxed)
    }

    fn platform_name(&self) -> &'static str {
        "youtube"
    }

    fn update_token(&mut self, token: String) {
        if let Some(tx) = &self.oauth_token_tx {
            if tx.send(token).is_err() {
                debug!("YouTube chat token update failed: receiver dropped");
            }
        }
    }

    async fn send_message(&mut self, message: String) -> PlatformResult<()> {
        if !self.can_send {
            return Err(PlatformError::Authentication(
                "YouTube account is not authenticated for sending".to_string(),
            ));
        }

        let live_chat_id = self
            .live_chat_id
            .clone()
            .ok_or_else(|| PlatformError::Platform("No active live chat ID".to_string()))?;

        let auth_mode = self.auth_mode.clone().ok_or_else(|| {
            PlatformError::Authentication("Missing authentication mode".to_string())
        })?;

        if matches!(auth_mode, AuthMode::ApiKey { .. }) {
            return Err(PlatformError::Authentication(
                "YouTube API key cannot send messages".to_string(),
            ));
        }

        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| PlatformError::Network(format!("Failed to create HTTP client: {}", e)))?;

        let url = format!("{}/liveChat/messages?part=snippet", self.api_base);
        let message_text = message.clone();
        let body = serde_json::json!({
            "snippet": {
                "liveChatId": live_chat_id,
                "type": "textMessageEvent",
                "textMessageDetails": { "messageText": message_text }
            }
        });

        let response = auth_mode
            .apply(http_client.post(&url))
            .json(&body)
            .send()
            .await
            .map_err(|e| PlatformError::Network(format!("Failed to send message: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|e| format!("<failed to read response body: {e}>"));
            // `INVALID_REQUEST_METADATA` on insert is YouTube refusing to
            // resolve WHICH channel is posting — the request body is valid
            // (reads work with the same token), so it's an identity binding
            // the API can't carry. Surface an actionable message instead of
            // the raw Google error blob; the fix is account-side re-consent.
            if status.as_u16() == 400 && body.contains("INVALID_REQUEST_METADATA") {
                return Err(PlatformError::Authentication(
                    "YouTube won't accept messages from this sign-in: it can't confirm which \
                     channel is posting. Sign out of YouTube here, sign back in, and pick the \
                     exact channel you stream from (for a Brand Account choose the brand channel, \
                     not your personal Google account)."
                        .to_string(),
                ));
            }
            return Err(PlatformError::Platform(format!(
                "YouTube send failed ({}): {}",
                status, body
            )));
        }

        if let Ok(mut recent) = self.recent_outbound.lock() {
            recent.push_back(OutboundMessage {
                text: message,
                timestamp: Instant::now(),
            });
            while recent.len() > 100 {
                recent.pop_front();
            }
        }

        Ok(())
    }

    fn can_send(&self) -> bool {
        self.can_send && self.is_connected()
    }

    fn last_error(&self) -> Option<String> {
        self.last_error.lock().ok().and_then(|e| e.clone())
    }
}

impl Default for YouTubeConnector {
    fn default() -> Self {
        Self::new()
    }
}
