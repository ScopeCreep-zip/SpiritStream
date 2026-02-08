use async_trait::async_trait;
use log::{info, warn, error, debug};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch};

use crate::models::{
    ChatConnectionStatus, ChatCredentials, ChatMessage,
    ChatPlatform as ChatPlatformEnum, YouTubeAuth,
};

use super::platform::{ChatPlatform, PlatformError, PlatformResult};

const YOUTUBE_API_BASE: &str = "https://www.googleapis.com/youtube/v3";
const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTING: u8 = 1;
const STATUS_CONNECTED: u8 = 2;
const STATUS_ERROR: u8 = 3;
const OUTBOUND_DEDUP_WINDOW_SECS: u64 = 10;

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

#[derive(Debug, Clone)]
struct OutboundMessage {
    text: String,
    timestamp: Instant,
}

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
}

impl YouTubeConnector {
    pub fn new() -> Self {
        Self {
            status: Arc::new(AtomicU8::new(STATUS_DISCONNECTED)),
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
        }
    }
}

/// Auth info extracted from credentials for API calls
#[derive(Clone)]
enum AuthMode {
    /// OAuth Bearer token shared via watch channel (supports live refresh)
    OAuth { access_token_rx: watch::Receiver<String> },
    /// API key query parameter
    ApiKey { key: String },
}

impl AuthMode {
    /// Apply auth to a reqwest RequestBuilder
    fn apply(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match self {
            AuthMode::OAuth { access_token_rx } => {
                let token = access_token_rx.borrow().clone();
                builder.header("Authorization", format!("Bearer {}", token))
            }
            AuthMode::ApiKey { key } => builder.query(&[("key", key.as_str())]),
        }
    }
}

/// Find the live chat ID for the active broadcast
async fn find_live_chat_id(
    client: &reqwest::Client,
    auth: &AuthMode,
    channel_id: &str,
) -> Result<String, PlatformError> {
    match auth {
        AuthMode::OAuth { .. } => {
            // OAuth mode: use liveBroadcasts.list with mine=true (5 quota units)
            let url = format!("{}/liveBroadcasts", YOUTUBE_API_BASE);
            let resp = auth
                .apply(client.get(&url))
                .query(&[
                    ("part", "snippet"),
                    ("broadcastStatus", "active"),
                    ("broadcastType", "all"),
                ])
                .send()
                .await
                .map_err(|e| PlatformError::Network(format!("Failed to fetch broadcasts: {}", e)))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                return Err(PlatformError::Platform(format!(
                    "YouTube API error ({}): {}",
                    status, body
                )));
            }

            let data: serde_json::Value = resp.json().await.map_err(|e| {
                PlatformError::Network(format!("Failed to parse broadcasts response: {}", e))
            })?;

            // Get the first active broadcast's liveChatId
            data["items"]
                .as_array()
                .and_then(|items| items.first())
                .and_then(|item| item["snippet"]["liveChatId"].as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| {
                    PlatformError::Platform(
                        "No active live broadcast found. Make sure you are currently live streaming on YouTube.".to_string(),
                    )
                })
        }
        AuthMode::ApiKey { .. } => {
            // API key mode: search for live videos, then get liveStreamingDetails
            // Step 1: Find live video for the channel (100 quota units)
            let search_url = format!("{}/search", YOUTUBE_API_BASE);
            let resp = auth
                .apply(client.get(&search_url))
                .query(&[
                    ("part", "id"),
                    ("channelId", channel_id),
                    ("type", "video"),
                    ("eventType", "live"),
                    ("maxResults", "1"),
                ])
                .send()
                .await
                .map_err(|e| PlatformError::Network(format!("Failed to search live videos: {}", e)))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                return Err(PlatformError::Platform(format!(
                    "YouTube API error ({}): {}",
                    status, body
                )));
            }

            let search_data: serde_json::Value = resp.json().await.map_err(|e| {
                PlatformError::Network(format!("Failed to parse search response: {}", e))
            })?;

            let video_id = search_data["items"]
                .as_array()
                .and_then(|items| items.first())
                .and_then(|item| item["id"]["videoId"].as_str())
                .ok_or_else(|| {
                    PlatformError::Platform(
                        "No active live stream found for this channel. Make sure the channel is currently live streaming.".to_string(),
                    )
                })?
                .to_string();

            // Step 2: Get liveStreamingDetails for the video (1 quota unit)
            let videos_url = format!("{}/videos", YOUTUBE_API_BASE);
            let resp = auth
                .apply(client.get(&videos_url))
                .query(&[
                    ("part", "liveStreamingDetails"),
                    ("id", &video_id),
                ])
                .send()
                .await
                .map_err(|e| PlatformError::Network(format!("Failed to fetch video details: {}", e)))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                return Err(PlatformError::Platform(format!(
                    "YouTube API error ({}): {}",
                    status, body
                )));
            }

            let video_data: serde_json::Value = resp.json().await.map_err(|e| {
                PlatformError::Network(format!("Failed to parse video response: {}", e))
            })?;

            video_data["items"]
                .as_array()
                .and_then(|items| items.first())
                .and_then(|item| item["liveStreamingDetails"]["activeLiveChatId"].as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| {
                    PlatformError::Platform(
                        "Live stream found but no active chat. Chat may be disabled for this stream.".to_string(),
                    )
                })
        }
    }
}

#[async_trait]
impl ChatPlatform for YouTubeConnector {
    async fn connect(
        &mut self,
        credentials: ChatCredentials,
        message_tx: mpsc::UnboundedSender<ChatMessage>,
    ) -> PlatformResult<()> {
        if self.is_connected() {
            return Err(PlatformError::AlreadyConnected);
        }

        self.status.store(status_to_u8(ChatConnectionStatus::Connecting), Ordering::Relaxed);
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
                        AuthMode::OAuth { access_token_rx: token_rx }
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

        info!("Connecting to YouTube Live Chat for channel: {}", channel_id);
        self.status.store(status_to_u8(ChatConnectionStatus::Connecting), Ordering::Relaxed);

        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| PlatformError::Network(format!("Failed to create HTTP client: {}", e)))?;

        // Find the active live chat ID
        let live_chat_id = match find_live_chat_id(&http_client, &auth_mode, &channel_id).await {
            Ok(id) => {
                info!("Found YouTube live chat ID");
                id
            }
            Err(e) => {
                self.status.store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
                if let Ok(mut guard) = self.last_error.lock() {
                    *guard = Some(format!("{}", e));
                }
                return Err(e);
            }
        };

        self.channel_id = Some(channel_id);
        self.live_chat_id = Some(live_chat_id.clone());
        self.auth_mode = Some(auth_mode.clone());
        self.can_send = matches!(auth_mode, AuthMode::OAuth { .. });
        self.self_channel_id = self.channel_id.clone();
        self.status.store(status_to_u8(ChatConnectionStatus::Connected), Ordering::Relaxed);

        // Create disconnect channel
        let (disconnect_tx, mut disconnect_rx) = mpsc::channel::<()>(1);
        self.disconnect_tx = Some(disconnect_tx);

        info!("Connected to YouTube Live Chat, starting polling");

        // Spawn the polling task
        let status = self.status.clone();
        let last_error = self.last_error.clone();
        let message_count = self.message_count.clone();
        let disconnecting = self.disconnecting.clone();
        let recent_outbound = self.recent_outbound.clone();
        let self_channel_id = self.self_channel_id.clone();
        tokio::spawn(async move {
            let mut page_token: Option<String> = None;
            // Start with a reasonable default; updated from API response
            let mut poll_interval_ms: u64 = 6000;

            loop {
                // Check for disconnect signal
                if disconnect_rx.try_recv().is_ok() {
                    info!("YouTube chat disconnect signal received");
                    disconnecting.store(true, Ordering::Relaxed);
                    break;
                }

                // Build the request
                let mut url = format!(
                    "{}/liveChat/messages?liveChatId={}&part=snippet,authorDetails&maxResults=200",
                    YOUTUBE_API_BASE, live_chat_id
                );
                if let Some(ref token) = page_token {
                    url.push_str(&format!("&pageToken={}", token));
                }

                let request = auth_mode.apply(http_client.get(&url));
                match request.send().await {
                    Ok(resp) => {
                        if !resp.status().is_success() {
                            let http_status = resp.status();
                            let body = resp.text().await.unwrap_or_default();
                            status.store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
                            // 403 often means quota exhausted; 401 means token expired
                            if http_status.as_u16() == 401 {
                                error!("YouTube API auth expired, waiting for token refresh");
                                poll_interval_ms = poll_interval_ms.max(30000);
                                if let Ok(mut guard) = last_error.lock() {
                                    *guard = Some("YouTube auth expired".to_string());
                                }
                            }
                            if http_status.as_u16() == 403 {
                                warn!("YouTube API quota may be exhausted (403). Backing off.");
                                poll_interval_ms = poll_interval_ms.max(30000);
                            } else {
                                warn!("YouTube chat API error ({}): {}", http_status, body);
                                if let Ok(mut guard) = last_error.lock() {
                                    *guard = Some(format!("YouTube API error {}", http_status));
                                }
                            }
                        } else {
                            match resp.json::<serde_json::Value>().await {
                                Ok(data) => {
                                    if let Ok(mut guard) = last_error.lock() {
                                        *guard = None;
                                    }
                                    status.store(status_to_u8(ChatConnectionStatus::Connected), Ordering::Relaxed);

                                    // Update polling interval from API recommendation
                                    if let Some(interval) = data["pollingIntervalMillis"].as_u64() {
                                        poll_interval_ms = interval;
                                    }

                                    // Update page token for next request
                                    page_token = data["nextPageToken"]
                                        .as_str()
                                        .map(|s| s.to_string());

                                    // Process messages
                                    if let Some(items) = data["items"].as_array() {
                                        for item in items {
                                            let snippet = &item["snippet"];
                                            let author = &item["authorDetails"];

                                            // Only process text messages
                                            let msg_type = snippet["type"].as_str().unwrap_or("");
                                            if msg_type != "textMessageEvent" {
                                                continue;
                                            }

                                            let username = author["displayName"]
                                                .as_str()
                                                .unwrap_or("Unknown")
                                                .to_string();
                                            let message_text = snippet["textMessageDetails"]["messageText"]
                                                .as_str()
                                                .unwrap_or("")
                                                .to_string();

                                            if message_text.is_empty() {
                                                continue;
                                            }

                                            if let Some(self_id) = &self_channel_id {
                                                if author["channelId"].as_str() == Some(self_id.as_str()) {
                                                    let mut recent = recent_outbound.lock().unwrap_or_else(|e| e.into_inner());
                                                    let now = Instant::now();
                                                    while let Some(front) = recent.front() {
                                                        if now.duration_since(front.timestamp).as_secs() > OUTBOUND_DEDUP_WINDOW_SECS {
                                                            recent.pop_front();
                                                        } else {
                                                            break;
                                                        }
                                                    }
                                                    if recent.iter().any(|entry| entry.text == message_text) {
                                                        continue;
                                                    }
                                                }
                                            }

                                            let mut chat_msg = ChatMessage::new(
                                                ChatPlatformEnum::YouTube,
                                                username,
                                                message_text,
                                            );
                                            if let Some(source_id) = item["id"].as_str() {
                                                chat_msg = chat_msg.with_source_id(source_id.to_string());
                                            }

                                            // Build badges from author details
                                            let mut badges = Vec::new();
                                            if author["isChatOwner"].as_bool().unwrap_or(false) {
                                                badges.push("owner".to_string());
                                            }
                                            if author["isChatModerator"].as_bool().unwrap_or(false) {
                                                badges.push("moderator".to_string());
                                            }
                                            if author["isChatSponsor"].as_bool().unwrap_or(false) {
                                                badges.push("member".to_string());
                                            }
                                            if !badges.is_empty() {
                                                chat_msg = chat_msg.with_badges(badges);
                                            }

                                            if message_tx.send(chat_msg).is_err() {
                                                warn!("Failed to send YouTube message: receiver dropped");
                                                return;
                                            }
                                        }

                                        if !items.is_empty() {
                                            debug!("Processed {} YouTube chat messages", items.len());
                                            message_count.fetch_add(items.len() as u64, Ordering::Relaxed);
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to parse YouTube chat response: {}", e);
                                    status.store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
                                    if let Ok(mut guard) = last_error.lock() {
                                        *guard = Some("Failed to parse YouTube chat response".to_string());
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("YouTube chat request failed: {}", e);
                        status.store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
                        if let Ok(mut guard) = last_error.lock() {
                            *guard = Some("YouTube chat request failed".to_string());
                        }
                    }
                }

                // Wait before next poll
                tokio::time::sleep(Duration::from_millis(poll_interval_ms)).await;
            }

            if !disconnecting.load(Ordering::Relaxed) {
                status.store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
                if let Ok(mut guard) = last_error.lock() {
                    *guard = Some("YouTube chat polling stopped".to_string());
                }
            }
            info!("YouTube chat polling stopped");
        });

        Ok(())
    }

    async fn disconnect(&mut self) -> PlatformResult<()> {
        if !self.is_connected() {
            return Err(PlatformError::NotConnected);
        }

        info!("Disconnecting from YouTube Live Chat");
        self.disconnecting.store(true, Ordering::Relaxed);

        // Send disconnect signal to polling task
        if let Some(tx) = self.disconnect_tx.take() {
            let _ = tx.send(()).await;
        }

        self.channel_id = None;
        self.self_channel_id = None;
        self.live_chat_id = None;
        self.auth_mode = None;
        self.can_send = false;
        self.status.store(status_to_u8(ChatConnectionStatus::Disconnected), Ordering::Relaxed);
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

        let auth_mode = self
            .auth_mode
            .clone()
            .ok_or_else(|| PlatformError::Authentication("Missing authentication mode".to_string()))?;

        if matches!(auth_mode, AuthMode::ApiKey { .. }) {
            return Err(PlatformError::Authentication(
                "YouTube API key cannot send messages".to_string(),
            ));
        }

        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| PlatformError::Network(format!("Failed to create HTTP client: {}", e)))?;

        let url = format!("{}/liveChat/messages?part=snippet", YOUTUBE_API_BASE);
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
            let body = response.text().await.unwrap_or_default();
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
