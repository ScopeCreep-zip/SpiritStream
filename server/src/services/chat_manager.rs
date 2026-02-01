use log::{error, info};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Mutex};

use crate::models::{
    ChatConfig, ChatMessage, ChatPlatform, ChatPlatformStatus,
};
use crate::services::chat::{ChatPlatform as ChatPlatformTrait, TikTokConnector, TwitchConnector};

/// Central manager for all chat platform connections
pub struct ChatManager {
    app_handle: AppHandle,
    platforms: Arc<Mutex<HashMap<ChatPlatform, Box<dyn ChatPlatformTrait>>>>,
    message_rx: Arc<Mutex<Option<mpsc::UnboundedReceiver<ChatMessage>>>>,
    message_tx: mpsc::UnboundedSender<ChatMessage>,
}

impl ChatManager {
    /// Create a new ChatManager
    pub fn new(app_handle: AppHandle) -> Self {
        let (message_tx, message_rx) = mpsc::unbounded_channel();

        let manager = Self {
            app_handle,
            platforms: Arc::new(Mutex::new(HashMap::new())),
            message_rx: Arc::new(Mutex::new(Some(message_rx))),
            message_tx,
        };

        // Start message handler
        manager.start_message_handler();

        manager
    }

    /// Initialize a platform connector
    fn create_platform_connector(platform: ChatPlatform) -> Box<dyn ChatPlatformTrait> {
        match platform {
            ChatPlatform::Twitch => Box::new(TwitchConnector::new()),
            ChatPlatform::TikTok => Box::new(TikTokConnector::new()),
            _ => {
                // For unimplemented platforms, return a stub
                // In the future, implement YouTube, Kick, Facebook connectors
                Box::new(TwitchConnector::new()) // Placeholder
            }
        }
    }

    /// Connect to a chat platform
    pub async fn connect(&self, config: ChatConfig) -> Result<(), String> {
        if !config.enabled {
            return Err("Platform is not enabled".to_string());
        }

        info!("Connecting to {} chat", config.platform.as_str());

        let mut platforms = self.platforms.lock().await;

        // Check if already connected
        if let Some(connector) = platforms.get(&config.platform) {
            if connector.is_connected() {
                return Err(format!(
                    "{} is already connected",
                    config.platform.as_str()
                ));
            }
        }

        // Create or get platform connector
        let mut connector = platforms
            .remove(&config.platform)
            .unwrap_or_else(|| Self::create_platform_connector(config.platform));

        // Connect to the platform
        let message_tx = self.message_tx.clone();
        connector
            .connect(config.credentials, message_tx)
            .await
            .map_err(|e| format!("Failed to connect to {}: {}", config.platform.as_str(), e))?;

        // Store the connector
        platforms.insert(config.platform, connector);

        info!("Successfully connected to {}", config.platform.as_str());
        Ok(())
    }

    /// Disconnect from a chat platform
    pub async fn disconnect(&self, platform: ChatPlatform) -> Result<(), String> {
        info!("Disconnecting from {} chat", platform.as_str());

        let mut platforms = self.platforms.lock().await;

        if let Some(mut connector) = platforms.remove(&platform) {
            connector
                .disconnect()
                .await
                .map_err(|e| format!("Failed to disconnect from {}: {}", platform.as_str(), e))?;

            // Re-insert the disconnected connector
            platforms.insert(platform, connector);

            info!("Successfully disconnected from {}", platform.as_str());
            Ok(())
        } else {
            Err(format!("{} is not connected", platform.as_str()))
        }
    }

    /// Disconnect from all platforms
    pub async fn disconnect_all(&self) -> Result<(), String> {
        info!("Disconnecting from all chat platforms");

        let mut platforms = self.platforms.lock().await;
        let mut errors = Vec::new();

        for (platform, connector) in platforms.iter_mut() {
            if connector.is_connected() {
                if let Err(e) = connector.disconnect().await {
                    errors.push(format!("{}: {}", platform.as_str(), e));
                }
            }
        }

        if errors.is_empty() {
            info!("Successfully disconnected from all platforms");
            Ok(())
        } else {
            Err(format!("Some disconnections failed: {}", errors.join(", ")))
        }
    }

    /// Get status of all platforms
    pub async fn get_status(&self) -> Vec<ChatPlatformStatus> {
        let platforms = self.platforms.lock().await;

        platforms
            .iter()
            .map(|(platform, connector)| ChatPlatformStatus {
                platform: *platform,
                status: connector.status(),
                message_count: connector.message_count(),
                error: None,
            })
            .collect()
    }

    /// Get status of a specific platform
    pub async fn get_platform_status(&self, platform: ChatPlatform) -> Option<ChatPlatformStatus> {
        let platforms = self.platforms.lock().await;

        platforms.get(&platform).map(|connector| ChatPlatformStatus {
            platform,
            status: connector.status(),
            message_count: connector.message_count(),
            error: None,
        })
    }

    /// Check if any platform is connected
    pub async fn is_any_connected(&self) -> bool {
        let platforms = self.platforms.lock().await;
        platforms.values().any(|c| c.is_connected())
    }

    /// Start the message handler that forwards messages to the frontend
    fn start_message_handler(&self) {
        let app_handle = self.app_handle.clone();
        let message_rx = self.message_rx.clone();

        // Use Tauri's async runtime instead of tokio::spawn
        tauri::async_runtime::spawn(async move {
            let rx = {
                let mut guard = message_rx.lock().await;
                guard.take()
            };

            if let Some(mut receiver) = rx {
                info!("Chat message handler started");

                while let Some(message) = receiver.recv().await {
                    // Emit message to frontend via Tauri events
                    if let Err(e) = app_handle.emit("chat_message", &message) {
                        error!("Failed to emit chat message to frontend: {}", e);
                    }
                }

                info!("Chat message handler stopped");
            } else {
                error!("Message receiver was already taken");
            }
        });
    }
}

/// Cleanup implementation
impl Drop for ChatManager {
    fn drop(&mut self) {
        info!("ChatManager dropped");
    }
}
