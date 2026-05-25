use log::info;

use crate::errors::CoreError;
use crate::models::{ChatConfig, ChatPlatform};
use crate::services::AuditAction;

use super::chat_validation;

impl super::ChatManager {
    /// Connect to a chat platform.
    pub async fn connect(&self, config: ChatConfig) -> Result<(), CoreError> {
        if !config.enabled {
            return Err(chat_validation(
                "chat_platform_not_enabled",
                "Platform is not enabled",
            ));
        }

        info!("Connecting to {} chat", config.platform.as_str());

        let mut platforms = self.platforms.lock().await;

        // Check if already connected.
        if let Some(connector) = platforms.get(&config.platform) {
            if connector.is_connected() {
                return Err(chat_validation(
                    "chat_platform_already_connected",
                    format!("{} is already connected", config.platform.as_str()),
                ));
            }
        }

        // Create or get platform connector. Unimplemented platforms
        // (Kick, Facebook) return None → clean validation error.
        let mut connector = match platforms.remove(&config.platform) {
            Some(existing) => existing,
            None => Self::create_platform_connector(config.platform).ok_or_else(|| {
                chat_validation(
                    "chat_platform_unsupported",
                    format!(
                        "Chat for {} is not yet implemented",
                        config.platform.as_str()
                    ),
                )
            })?,
        };

        // Connect to the platform. The connector itself tracks its own
        // `last_error` (see `ChatPlatform::last_error` trait impls); re-
        // inserting the failed connector lets the UI surface that error
        // through `get_platform_status` without a separate stale cache.
        let message_tx = self.message_tx.clone();
        if let Err(e) = connector.connect(config.credentials, message_tx).await {
            let error = format!("Failed to connect to {}: {}", config.platform.as_str(), e);
            platforms.insert(config.platform, connector);
            return Err(CoreError::Internal { context: error });
        }

        platforms.insert(config.platform, connector);

        info!("Successfully connected to {}", config.platform.as_str());

        if let Some(audit) = self.audit() {
            let _ = audit.record(AuditAction::ChatPlatformConnected {
                platform: config.platform.as_str().to_string(),
                account_id: None,
            });
        }

        Ok(())
    }

    /// Disconnect from a chat platform.
    pub async fn disconnect(&self, platform: ChatPlatform) -> Result<(), CoreError> {
        info!("Disconnecting from {} chat", platform.as_str());

        let mut platforms = self.platforms.lock().await;

        if let Some(mut connector) = platforms.remove(&platform) {
            connector
                .disconnect()
                .await
                .map_err(|e| CoreError::Internal {
                    context: format!("Failed to disconnect from {}: {}", platform.as_str(), e),
                })?;

            // Re-insert the disconnected connector.
            platforms.insert(platform, connector);

            info!("Successfully disconnected from {}", platform.as_str());

            if let Some(audit) = self.audit() {
                let _ = audit.record(AuditAction::ChatPlatformDisconnected {
                    platform: platform.as_str().to_string(),
                    reason: "user_requested".to_string(),
                });
            }

            Ok(())
        } else {
            Err(CoreError::ChatPlatformNotConnected {
                platform: platform.as_str().to_string(),
            })
        }
    }

    /// Update the OAuth access token for a specific platform (if connected).
    pub async fn update_platform_token(
        &self,
        platform: ChatPlatform,
        token: String,
    ) -> Result<(), CoreError> {
        let mut platforms = self.platforms.lock().await;

        if let Some(connector) = platforms.get_mut(&platform) {
            connector.update_token(token);
            Ok(())
        } else {
            Err(CoreError::ChatPlatformNotConnected {
                platform: platform.as_str().to_string(),
            })
        }
    }

    /// Disconnect from every connected platform. `reason` is recorded
    /// against each per-platform `ChatPlatformDisconnected` audit entry
    /// so a forensic reader can distinguish a panic teardown from a
    /// user-requested mass-disconnect from a process shutdown. Use the
    /// stable short discriminators documented on the audit variant:
    /// `"panic_triggered"`, `"user_requested"`, `"shutdown"`.
    pub async fn disconnect_all(&self, reason: &str) -> Result<(), CoreError> {
        info!("Disconnecting from all chat platforms (reason={reason})");

        let mut platforms = self.platforms.lock().await;
        let mut errors = Vec::new();
        let mut disconnected: Vec<ChatPlatform> = Vec::new();

        for (platform, connector) in platforms.iter_mut() {
            if connector.is_connected() {
                match connector.disconnect().await {
                    Ok(()) => disconnected.push(*platform),
                    Err(e) => errors.push(format!("{}: {}", platform.as_str(), e)),
                }
            }
        }

        if let Some(audit) = self.audit() {
            for platform in &disconnected {
                let _ = audit.record(AuditAction::ChatPlatformDisconnected {
                    platform: platform.as_str().to_string(),
                    reason: reason.to_string(),
                });
            }
        }

        if errors.is_empty() {
            info!("Successfully disconnected from all platforms");
            Ok(())
        } else {
            Err(CoreError::Internal {
                context: format!("Some disconnections failed: {}", errors.join(", ")),
            })
        }
    }
}
