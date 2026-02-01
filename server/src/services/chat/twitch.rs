use async_trait::async_trait;
use log::{info, warn, error};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::message::ServerMessage;
use twitch_irc::{ClientConfig, SecureTCPTransport, TwitchIRCClient};

use crate::models::{ChatConnectionStatus, ChatCredentials, ChatMessage, ChatPlatform as ChatPlatformEnum};

use super::platform::{ChatPlatform, PlatformError, PlatformResult};

/// Validate a Twitch channel exists by checking the Twitch API
async fn validate_channel_exists(channel: &str) -> Result<bool, String> {
    // Use Twitch's public endpoint that doesn't require authentication
    // This endpoint returns 200 for valid channels and 404 for invalid ones
    let url = format!("https://www.twitch.tv/{}", channel);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .head(&url)
        .header("User-Agent", "SpiritStream/1.0")
        .send()
        .await
        .map_err(|e| format!("Failed to check channel: {}", e))?;

    // If the channel exists, we get a 200. If not, we get a redirect to 404 page
    // We can also check if content contains channel info, but HEAD is faster
    Ok(response.status().is_success())
}

type TwitchClient = TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>;

/// Twitch IRC chat connector
pub struct TwitchConnector {
    client: Option<Arc<TwitchClient>>,
    status: ChatConnectionStatus,
    message_count: u64,
    channel: Option<String>,
}

impl TwitchConnector {
    pub fn new() -> Self {
        Self {
            client: None,
            status: ChatConnectionStatus::Disconnected,
            message_count: 0,
            channel: None,
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

        // Extract Twitch credentials
        let (channel, oauth_token) = match credentials {
            ChatCredentials::Twitch { channel, oauth_token } => (channel, oauth_token),
            _ => {
                return Err(PlatformError::InvalidConfig(
                    "Expected Twitch credentials".to_string(),
                ))
            }
        };

        info!("Connecting to Twitch channel: {}", channel);
        self.status = ChatConnectionStatus::Connecting;

        // Validate channel exists before connecting
        let channel_lower = channel.to_lowercase();
        match validate_channel_exists(&channel_lower).await {
            Ok(true) => {
                info!("Twitch channel '{}' validated successfully", channel_lower);
            }
            Ok(false) => {
                error!("Twitch channel '{}' does not exist", channel_lower);
                self.status = ChatConnectionStatus::Error;
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
        let login_credentials = if let Some(_token) = oauth_token {
            // For authenticated connection, you'd parse username from token
            // For now, we'll use anonymous connection
            StaticLoginCredentials::anonymous()
        } else {
            StaticLoginCredentials::anonymous()
        };

        // Create client config
        let config = ClientConfig::new_simple(login_credentials);
        let (mut incoming_messages, client) = TwitchIRCClient::<SecureTCPTransport, StaticLoginCredentials>::new(config);

        // Join the channel
        client.join(channel_lower.clone()).map_err(|e| {
            PlatformError::Connection(format!("Failed to join channel: {}", e))
        })?;

        self.client = Some(Arc::new(client));
        self.channel = Some(channel_lower.clone());
        self.status = ChatConnectionStatus::Connected;

        info!("Connected to Twitch channel: {}", channel_lower);

        // Spawn task to handle incoming messages
        let message_count = Arc::new(Mutex::new(0u64));
        let message_count_clone = message_count.clone();

        tokio::spawn(async move {
            while let Some(message) = incoming_messages.recv().await {
                match message {
                    ServerMessage::Privmsg(msg) => {
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
                        .with_color(color);

                        // Add badges if available
                        if let Some(badges) = Self::parse_badges(&msg.sender) {
                            let chat_message = chat_message.with_badges(badges);
                            if message_tx.send(chat_message).is_err() {
                                warn!("Failed to send Twitch message: receiver dropped");
                                break;
                            }
                        } else {
                            if message_tx.send(chat_message).is_err() {
                                warn!("Failed to send Twitch message: receiver dropped");
                                break;
                            }
                        }

                        // Increment message count
                        let mut count = message_count_clone.lock().await;
                        *count += 1;
                    }
                    ServerMessage::Notice(notice) => {
                        info!("Twitch notice: {}", notice.message_text);
                    }
                    ServerMessage::Reconnect(_) => {
                        warn!("Twitch server requested reconnect");
                    }
                    _ => {
                        // Ignore other message types
                    }
                }
            }
            info!("Twitch message handler stopped");
        });

        // Store message count reference
        let count = message_count.lock().await;
        self.message_count = *count;

        Ok(())
    }

    async fn disconnect(&mut self) -> PlatformResult<()> {
        if !self.is_connected() {
            return Err(PlatformError::NotConnected);
        }

        info!("Disconnecting from Twitch");

        if let Some(channel) = &self.channel {
            if let Some(client) = &self.client {
                // Part from the channel
                client.part(channel.clone());
            }
        }

        self.client = None;
        self.channel = None;
        self.status = ChatConnectionStatus::Disconnected;

        info!("Disconnected from Twitch");
        Ok(())
    }

    fn status(&self) -> ChatConnectionStatus {
        self.status
    }

    fn message_count(&self) -> u64 {
        self.message_count
    }

    fn platform_name(&self) -> &'static str {
        "twitch"
    }
}

impl Default for TwitchConnector {
    fn default() -> Self {
        Self::new()
    }
}
