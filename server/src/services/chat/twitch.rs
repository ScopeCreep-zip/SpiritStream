use async_trait::async_trait;
use log::{info, warn, error};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::message::ServerMessage;
use twitch_irc::{ClientConfig, SecureTCPTransport, TwitchIRCClient};

use crate::models::{ChatConnectionStatus, ChatCredentials, ChatMessage, ChatPlatform as ChatPlatformEnum, TwitchAuth};

use super::platform::{ChatPlatform, PlatformError, PlatformResult};

/// Validate a Twitch channel exists using Twitch's public GraphQL API
async fn validate_channel_exists(channel: &str) -> Result<bool, String> {
    // Use Twitch's public GQL endpoint - no auth required for basic channel lookup
    let url = "https://gql.twitch.tv/gql";

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // GraphQL query to check if channel exists
    let query = serde_json::json!({
        "query": format!(
            r#"query {{ user(login: "{}") {{ id login displayName }} }}"#,
            channel.to_lowercase()
        )
    });

    let response = client
        .post(url)
        .header("Client-Id", "kimne78kx3ncx6brgo4mv6wki5h1ko") // Public web client ID
        .header("Content-Type", "application/json")
        .json(&query)
        .send()
        .await
        .map_err(|e| format!("Failed to check channel: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Twitch API returned status: {}", response.status()));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    // Check if user exists in the response
    let user_exists = body
        .get("data")
        .and_then(|d| d.get("user"))
        .map(|u| !u.is_null())
        .unwrap_or(false);

    Ok(user_exists)
}

type TwitchClient = TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>;

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

/// Twitch IRC chat connector
pub struct TwitchConnector {
    client: Option<Arc<TwitchClient>>,
    status: Arc<AtomicU8>,
    last_error: Arc<StdMutex<Option<String>>>,
    message_count: Arc<AtomicU64>,
    disconnecting: Arc<AtomicBool>,
    can_send: bool,
    channel: Option<String>,
    self_login: Option<String>,
    recent_outbound: Arc<StdMutex<VecDeque<OutboundMessage>>>,
}

impl TwitchConnector {
    pub fn new() -> Self {
        Self {
            client: None,
            status: Arc::new(AtomicU8::new(STATUS_DISCONNECTED)),
            last_error: Arc::new(StdMutex::new(None)),
            message_count: Arc::new(AtomicU64::new(0)),
            disconnecting: Arc::new(AtomicBool::new(false)),
            can_send: false,
            channel: None,
            self_login: None,
            recent_outbound: Arc::new(StdMutex::new(VecDeque::new())),
        }
    }

    fn parse_badges(_tags: &twitch_irc::message::TwitchUserBasics) -> Option<Vec<String>> {
        let badges = Vec::new();

        // Note: The twitch-irc crate doesn't expose badges directly in TwitchUserBasics
        // We'd need to access the raw IRCMessage for that. For now, we'll return None.
        // In a more complete implementation, you'd parse the raw badges tag.

        if badges.is_empty() {
            None
        } else {
            Some(badges)
        }
    }

    async fn validate_oauth_user(token: &str) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct ValidateResponse {
            login: String,
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        let response = client
            .get("https://id.twitch.tv/oauth2/validate")
            .header("Authorization", format!("OAuth {}", token))
            .send()
            .await
            .map_err(|e| format!("Failed to validate token: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Token validation failed: {} {}", status, body));
        }

        let data: ValidateResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse validation response: {}", e))?;

        Ok(data.login)
    }
}

#[async_trait]
impl ChatPlatform for TwitchConnector {
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
        self.self_login = None;
        self.message_count.store(0, Ordering::Relaxed);
        if let Ok(mut recent) = self.recent_outbound.lock() {
            recent.clear();
        }
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = None;
        }

        // Extract Twitch credentials
        let (channel, oauth_token) = match credentials {
            ChatCredentials::Twitch { channel, auth } => {
                // Extract OAuth token from auth method if provided
                let token = auth.map(|a| match a {
                    TwitchAuth::UserToken { oauth_token } => oauth_token,
                    TwitchAuth::AppOAuth { access_token, .. } => access_token,
                });
                (channel, token)
            }
            _ => {
                return Err(PlatformError::InvalidConfig(
                    "Expected Twitch credentials".to_string(),
                ))
            }
        };

        info!("Connecting to Twitch channel: {}", channel);

        // Validate channel exists before connecting
        let channel_lower = channel.to_lowercase();
        match validate_channel_exists(&channel_lower).await {
            Ok(true) => {
                info!("Twitch channel '{}' validated successfully", channel_lower);
            }
            Ok(false) => {
                error!("Twitch channel '{}' does not exist", channel_lower);
                self.status.store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
                if let Ok(mut guard) = self.last_error.lock() {
                    *guard = Some(format!("Channel '{}' does not exist on Twitch", channel_lower));
                }
                return Err(PlatformError::InvalidConfig(format!(
                    "Channel '{}' does not exist on Twitch",
                    channel_lower
                )));
            }
            Err(e) => {
                // If validation fails (network issue), log warning but continue
                // Better to try connecting than to fail completely
                warn!("Could not validate Twitch channel '{}': {}. Attempting connection anyway.", channel_lower, e);
            }
        }

        // Create login credentials (anonymous if no OAuth token provided)
        let mut oauth_login: Option<String> = None;
        let mut clean_token: Option<String> = None;
        if let Some(token) = oauth_token {
            let token_clean = token.strip_prefix("oauth:").unwrap_or(&token).to_string();
            clean_token = Some(token_clean.clone());
            match Self::validate_oauth_user(&token_clean).await {
                Ok(login) => {
                    info!("Using authenticated connection for Twitch as {}", login);
                    oauth_login = Some(login.clone());
                    self.self_login = Some(login);
                    self.can_send = true;
                }
                Err(e) => {
                    warn!("Twitch token validation failed, falling back to read-only: {}", e);
                }
            }
        }

        let login_credentials = if let (Some(login), Some(token)) = (oauth_login.clone(), clean_token.clone()) {
            StaticLoginCredentials::new(login, Some(token))
        } else {
            info!("Using anonymous connection for Twitch (read-only)");
            StaticLoginCredentials::anonymous()
        };

        // Create client config
        let config = ClientConfig::new_simple(login_credentials);
        let (mut incoming_messages, client) = TwitchIRCClient::<SecureTCPTransport, StaticLoginCredentials>::new(config);

        // Join the channel
        if let Err(e) = client.join(channel_lower.clone()) {
            self.status.store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
            if let Ok(mut guard) = self.last_error.lock() {
                *guard = Some(format!("Failed to join channel: {}", e));
            }
            return Err(PlatformError::Connection(format!("Failed to join channel: {}", e)));
        }

        self.client = Some(Arc::new(client));
        self.channel = Some(channel_lower.clone());
        self.status.store(status_to_u8(ChatConnectionStatus::Connected), Ordering::Relaxed);

        info!("Connected to Twitch channel: {}", channel_lower);

        // Spawn task to handle incoming messages
        let message_count_clone = self.message_count.clone();
        let recent_outbound = self.recent_outbound.clone();
        let self_login = self.self_login.clone();
        let status = self.status.clone();
        let last_error = self.last_error.clone();
        let disconnecting = self.disconnecting.clone();

        tokio::spawn(async move {
            while let Some(message) = incoming_messages.recv().await {
                match message {
                    ServerMessage::Privmsg(msg) => {
                        if let Some(login) = &self_login {
                            if msg.sender.login.eq_ignore_ascii_case(login) {
                                let mut recent = recent_outbound.lock().unwrap_or_else(|e| e.into_inner());
                                let now = Instant::now();
                                while let Some(front) = recent.front() {
                                    if now.duration_since(front.timestamp).as_secs() > OUTBOUND_DEDUP_WINDOW_SECS {
                                        recent.pop_front();
                                    } else {
                                        break;
                                    }
                                }
                                if recent.iter().any(|entry| entry.text == msg.message_text) {
                                    continue;
                                }
                            }
                        }

                        // Convert RGBColor to hex string
                        let color = msg.name_color
                            .as_ref()
                            .map(|c| format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b))
                            .unwrap_or_else(|| "#9146FF".to_string());

                        let chat_message = ChatMessage::new(
                            ChatPlatformEnum::Twitch,
                            msg.sender.name.clone(),
                            msg.message_text.clone(),
                        )
                        .with_color(color)
                        .with_source_id(msg.message_id.clone());

                        // Add badges if available
                        if let Some(badges) = Self::parse_badges(&msg.sender) {
                            let chat_message = chat_message.with_badges(badges);
                            if message_tx.send(chat_message).is_err() {
                                warn!("Failed to send Twitch message: receiver dropped");
                                break;
                            }
                        } else if message_tx.send(chat_message).is_err() {
                            warn!("Failed to send Twitch message: receiver dropped");
                            break;
                        }

                        // Increment message count
                        message_count_clone.fetch_add(1, Ordering::Relaxed);
                    }
                    ServerMessage::Notice(notice) => {
                        info!("Twitch notice: {}", notice.message_text);
                    }
                    ServerMessage::Reconnect(_) => {
                        warn!("Twitch server requested reconnect");
                        status.store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
                        if let Ok(mut guard) = last_error.lock() {
                            *guard = Some("Twitch server requested reconnect".to_string());
                        }
                    }
                    _ => {
                        // Ignore other message types
                    }
                }
            }
            if !disconnecting.load(Ordering::Relaxed) {
                status.store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
                if let Ok(mut guard) = last_error.lock() {
                    *guard = Some("Twitch connection lost".to_string());
                }
            }
            info!("Twitch message handler stopped");
        });

        Ok(())
    }

    async fn disconnect(&mut self) -> PlatformResult<()> {
        if !self.is_connected() {
            return Err(PlatformError::NotConnected);
        }

        info!("Disconnecting from Twitch");
        self.disconnecting.store(true, Ordering::Relaxed);

        if let Some(channel) = &self.channel {
            if let Some(client) = &self.client {
                // Part from the channel
                client.part(channel.clone());
            }
        }

        self.client = None;
        self.channel = None;
        self.status.store(status_to_u8(ChatConnectionStatus::Disconnected), Ordering::Relaxed);
        self.can_send = false;
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = None;
        }

        info!("Disconnected from Twitch");
        Ok(())
    }

    fn status(&self) -> ChatConnectionStatus {
        status_from_u8(self.status.load(Ordering::Relaxed))
    }

    fn message_count(&self) -> u64 {
        self.message_count.load(Ordering::Relaxed)
    }

    fn platform_name(&self) -> &'static str {
        "twitch"
    }

    async fn send_message(&mut self, message: String) -> PlatformResult<()> {
        if !self.can_send {
            return Err(PlatformError::Authentication("Twitch account is not authenticated for sending".to_string()));
        }

        let client = self.client.as_ref().ok_or(PlatformError::NotConnected)?;
        let channel = self
            .channel
            .as_ref()
            .ok_or_else(|| PlatformError::Platform("No channel configured".to_string()))?
            .clone();

        client
            .say(channel, message.clone())
            .await
            .map_err(|e| PlatformError::Platform(format!("Failed to send message: {}", e)))?;

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

impl Default for TwitchConnector {
    fn default() -> Self {
        Self::new()
    }
}
