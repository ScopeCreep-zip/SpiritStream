use tokio::sync::watch;

use super::super::platform::PlatformError;

/// Auth info extracted from credentials for API calls
#[derive(Clone)]
pub(super) enum AuthMode {
    /// OAuth Bearer token shared via watch channel (supports live refresh)
    OAuth {
        access_token_rx: watch::Receiver<String>,
    },
    /// API key query parameter
    ApiKey { key: String },
}

impl AuthMode {
    /// Apply auth to a reqwest RequestBuilder
    pub(super) fn apply(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
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
pub(super) async fn find_live_chat_id(
    client: &reqwest::Client,
    auth: &AuthMode,
    channel_id: &str,
    api_base: &str,
) -> Result<String, PlatformError> {
    match auth {
        AuthMode::OAuth { .. } => {
            // OAuth mode: use liveBroadcasts.list with mine=true (5 quota units)
            let url = format!("{}/liveBroadcasts", api_base);
            let resp = auth
                .apply(client.get(&url))
                .query(&[
                    ("part", "snippet"),
                    ("broadcastStatus", "active"),
                    ("broadcastType", "all"),
                ])
                .send()
                .await
                .map_err(|e| {
                    PlatformError::Network(format!("Failed to fetch broadcasts: {}", e))
                })?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp
                    .text()
                    .await
                    .unwrap_or_else(|e| format!("<failed to read response body: {e}>"));
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
            // Find live video for the channel (100 quota units)
            let search_url = format!("{}/search", api_base);
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
                .map_err(|e| {
                    PlatformError::Network(format!("Failed to search live videos: {}", e))
                })?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp
                    .text()
                    .await
                    .unwrap_or_else(|e| format!("<failed to read response body: {e}>"));
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

            // Get liveStreamingDetails for the video (1 quota unit)
            let videos_url = format!("{}/videos", api_base);
            let resp = auth
                .apply(client.get(&videos_url))
                .query(&[("part", "liveStreamingDetails"), ("id", &video_id)])
                .send()
                .await
                .map_err(|e| {
                    PlatformError::Network(format!("Failed to fetch video details: {}", e))
                })?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp
                    .text()
                    .await
                    .unwrap_or_else(|e| format!("<failed to read response body: {e}>"));
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
