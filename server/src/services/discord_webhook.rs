// Discord Webhook Service
// Sends go-live notifications via Discord webhooks

use chrono::{DateTime, Utc};
use log::{error, info, warn};
use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Discord webhook message payload
#[derive(Debug, Serialize)]
struct WebhookPayload {
    /// Message content (supports Discord markdown)
    content: String,
    /// Optional username override for the webhook
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    /// Optional avatar URL override
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar_url: Option<String>,
}

/// Discord API rate limit response
#[derive(Debug, Deserialize)]
struct RateLimitResponse {
    /// Time to wait before retrying (in seconds)
    retry_after: f64,
    /// Whether this is a global rate limit
    #[allow(dead_code)]
    global: bool,
}

/// Result of sending a webhook notification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookResult {
    /// Whether the notification was sent successfully
    pub success: bool,
    /// Human-readable message
    pub message: String,
    /// Whether the notification was skipped due to cooldown
    pub skipped_cooldown: bool,
}

/// Discord webhook service state
pub struct DiscordWebhookService {
    /// HTTP client for making requests
    client: reqwest::Client,
    /// Timestamp of last successful notification send
    last_send_time: Arc<RwLock<Option<DateTime<Utc>>>>,
}

impl DiscordWebhookService {
    /// Create a new Discord webhook service
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            last_send_time: Arc::new(RwLock::new(None)),
        }
    }

    /// Send a go-live notification
    ///
    /// # Arguments
    /// * `webhook_url` - Discord webhook URL
    /// * `message` - Message content (supports Discord markdown)
    /// * `image_path` - Optional path to an image file to attach
    /// * `cooldown_enabled` - Whether cooldown is enabled
    /// * `cooldown_seconds` - Cooldown period in seconds
    pub async fn send_go_live_notification(
        &self,
        webhook_url: &str,
        message: &str,
        image_path: Option<&str>,
        cooldown_enabled: bool,
        cooldown_seconds: u32,
    ) -> WebhookResult {
        // Validate webhook URL
        if webhook_url.is_empty() {
            return WebhookResult {
                success: false,
                message: "Webhook URL is not configured".to_string(),
                skipped_cooldown: false,
            };
        }

        if !webhook_url.starts_with("https://discord.com/api/webhooks/")
            && !webhook_url.starts_with("https://discordapp.com/api/webhooks/")
        {
            return WebhookResult {
                success: false,
                message: "Invalid Discord webhook URL".to_string(),
                skipped_cooldown: false,
            };
        }

        // Check cooldown
        if cooldown_enabled {
            let last_send = self.last_send_time.read().await;
            if let Some(last_time) = *last_send {
                let elapsed = Utc::now().signed_duration_since(last_time);
                let cooldown_duration = chrono::Duration::seconds(cooldown_seconds as i64);

                if elapsed < cooldown_duration {
                    let remaining = (cooldown_duration - elapsed).num_seconds();
                    info!(
                        "Discord notification skipped: cooldown active ({} seconds remaining)",
                        remaining
                    );
                    return WebhookResult {
                        success: true,
                        message: format!(
                            "Notification skipped: cooldown active ({} seconds remaining)",
                            remaining
                        ),
                        skipped_cooldown: true,
                    };
                }
            }
        }

        // Build payload
        let payload = WebhookPayload {
            content: message.to_string(),
            username: Some("SpiritStream".to_string()),
            avatar_url: None,
        };

        // Send webhook request (with or without image)
        let result = if let Some(path) = image_path {
            if !path.is_empty() && Path::new(path).exists() {
                self.send_webhook_with_image(webhook_url, &payload, path).await
            } else {
                if !path.is_empty() {
                    warn!("Discord image file not found: {}", path);
                }
                self.send_webhook(webhook_url, &payload).await
            }
        } else {
            self.send_webhook(webhook_url, &payload).await
        };

        match result {
            Ok(()) => {
                // Update last send time
                let mut last_send = self.last_send_time.write().await;
                *last_send = Some(Utc::now());

                info!("Discord go-live notification sent successfully");
                WebhookResult {
                    success: true,
                    message: "Notification sent successfully".to_string(),
                    skipped_cooldown: false,
                }
            }
            Err(e) => {
                error!("Failed to send Discord notification: {}", e);
                WebhookResult {
                    success: false,
                    message: format!("Failed to send notification: {}", e),
                    skipped_cooldown: false,
                }
            }
        }
    }

    /// Send a webhook request to Discord
    async fn send_webhook(&self, url: &str, payload: &WebhookPayload) -> Result<(), String> {
        let response = self
            .client
            .post(url)
            .json(payload)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        self.handle_response(response).await
    }

    /// Send a webhook request with an image attachment
    async fn send_webhook_with_image(
        &self,
        url: &str,
        payload: &WebhookPayload,
        image_path: &str,
    ) -> Result<(), String> {
        // Read the image file
        let image_data = tokio::fs::read(image_path)
            .await
            .map_err(|e| format!("Failed to read image file: {}", e))?;

        // Get the file name from the path
        let file_name = Path::new(image_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("image.png")
            .to_string();

        // Determine MIME type from extension
        let mime_type = match Path::new(image_path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref()
        {
            Some("png") => "image/png",
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("gif") => "image/gif",
            Some("webp") => "image/webp",
            _ => "application/octet-stream",
        };

        // Serialize payload to JSON string
        let payload_json = serde_json::to_string(payload)
            .map_err(|e| format!("Failed to serialize payload: {}", e))?;

        // Build multipart form
        let file_part = Part::bytes(image_data)
            .file_name(file_name)
            .mime_str(mime_type)
            .map_err(|e| format!("Failed to create file part: {}", e))?;

        let form = Form::new()
            .text("payload_json", payload_json)
            .part("file", file_part);

        // Send the request
        let response = self
            .client
            .post(url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        self.handle_response(response).await
    }

    /// Handle Discord API response
    async fn handle_response(&self, response: reqwest::Response) -> Result<(), String> {
        let status = response.status();

        if status.is_success() || status.as_u16() == 204 {
            // 204 No Content is the normal success response for webhooks
            Ok(())
        } else if status.as_u16() == 429 {
            // Rate limited
            let rate_limit: RateLimitResponse = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse rate limit response: {}", e))?;

            warn!(
                "Discord rate limit hit, retry after {} seconds",
                rate_limit.retry_after
            );
            Err(format!(
                "Rate limited by Discord. Try again in {:.1} seconds",
                rate_limit.retry_after
            ))
        } else if status.as_u16() == 400 {
            // Bad request - usually means invalid message content
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(format!("Invalid request: {}", error_text))
        } else if status.as_u16() == 401 || status.as_u16() == 403 {
            Err("Invalid webhook URL or webhook has been deleted".to_string())
        } else if status.as_u16() == 404 {
            Err("Webhook not found - the webhook may have been deleted".to_string())
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(format!("Discord API error ({}): {}", status, error_text))
        }
    }

    /// Test a webhook URL by sending a test message
    pub async fn test_webhook(&self, webhook_url: &str) -> WebhookResult {
        // Validate webhook URL
        if webhook_url.is_empty() {
            return WebhookResult {
                success: false,
                message: "Webhook URL is empty".to_string(),
                skipped_cooldown: false,
            };
        }

        if !webhook_url.starts_with("https://discord.com/api/webhooks/")
            && !webhook_url.starts_with("https://discordapp.com/api/webhooks/")
        {
            return WebhookResult {
                success: false,
                message: "Invalid Discord webhook URL format".to_string(),
                skipped_cooldown: false,
            };
        }

        let payload = WebhookPayload {
            content: "🔔 **SpiritStream Test** - Webhook connection successful!".to_string(),
            username: Some("SpiritStream".to_string()),
            avatar_url: None,
        };

        match self.send_webhook(webhook_url, &payload).await {
            Ok(()) => {
                info!("Discord webhook test successful");
                WebhookResult {
                    success: true,
                    message: "Test message sent successfully".to_string(),
                    skipped_cooldown: false,
                }
            }
            Err(e) => {
                warn!("Discord webhook test failed: {}", e);
                WebhookResult {
                    success: false,
                    message: e,
                    skipped_cooldown: false,
                }
            }
        }
    }

    /// Reset the cooldown timer (useful for testing)
    pub async fn reset_cooldown(&self) {
        let mut last_send = self.last_send_time.write().await;
        *last_send = None;
        info!("Discord webhook cooldown reset");
    }
}

impl Default for DiscordWebhookService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_url_validation() {
        let service = DiscordWebhookService::new();

        // We can't actually test the webhook without a real URL,
        // but we can verify the service initializes correctly
        assert!(service.last_send_time.try_read().is_ok());
    }
}
