// go2rtc Client Service
// Manages communication with the go2rtc WebRTC media server

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

const DEFAULT_GO2RTC_URL: &str = "http://127.0.0.1:1984";
const REQUEST_TIMEOUT_SECS: u64 = 5;

/// Information about a WebRTC stream endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebRtcInfo {
    /// Whether go2rtc is available
    pub available: bool,
    /// WHEP URL for WebRTC playback
    pub whep_url: Option<String>,
    /// WebSocket URL for MSE/MJPEG playback
    pub ws_url: Option<String>,
    /// Stream name in go2rtc
    pub stream_name: Option<String>,
}

/// go2rtc stream configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Go2RtcStream {
    name: Option<String>,
    producers: Option<Vec<Go2RtcProducer>>,
    consumers: Option<Vec<Go2RtcConsumer>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Go2RtcProducer {
    url: Option<String>,
    #[serde(rename = "type")]
    producer_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Go2RtcConsumer {
    url: Option<String>,
}

/// Response from go2rtc /api/streams endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StreamsResponse(HashMap<String, Go2RtcStream>);

/// Client for interacting with go2rtc API
pub struct Go2RtcClient {
    client: Client,
    base_url: String,
}

impl Go2RtcClient {
    /// Create a new go2rtc client with the default URL
    pub fn new() -> Self {
        Self::with_url(DEFAULT_GO2RTC_URL.to_string())
    }

    /// Create a new go2rtc client with a custom URL
    pub fn with_url(base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .expect("Failed to create HTTP client");

        Self { client, base_url }
    }

    /// Check if go2rtc is available and healthy
    pub async fn health_check(&self) -> bool {
        match self.client.get(format!("{}/api", self.base_url)).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    /// Register an FFmpeg stream with go2rtc
    ///
    /// # Arguments
    /// * `name` - Unique stream name (typically source ID)
    /// * `source` - FFmpeg source URL or device string
    ///
    /// # Returns
    /// The WHEP URL for WebRTC playback on success
    pub async fn register_stream(&self, name: &str, source: &str) -> Result<String, String> {
        // go2rtc expects the source to be URL-encoded
        let encoded_source = urlencoding::encode(source);
        let url = format!("{}/api/streams?src={}&name={}", self.base_url, encoded_source, name);

        let response = self.client
            .put(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to register stream: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("go2rtc returned {}: {}", status, body));
        }

        log::info!("Registered stream '{}' with go2rtc: {}", name, source);
        Ok(self.get_whep_url(name))
    }

    /// Register an FFmpeg stream with exec source (command line)
    pub async fn register_ffmpeg_stream(&self, name: &str, ffmpeg_args: &[String]) -> Result<String, String> {
        // Build exec source: exec:ffmpeg <args>
        let args_str = ffmpeg_args.join(" ");
        let source = format!("exec:ffmpeg {}", args_str);
        self.register_stream(name, &source).await
    }

    /// Register an empty stream (placeholder for incoming push)
    /// This creates a stream that can receive data via RTSP or HTTP push
    pub async fn register_empty_stream(&self, name: &str) -> Result<String, String> {
        // Register with empty source - go2rtc will accept incoming pushes to this stream
        let url = format!("{}/api/streams?name={}", self.base_url, name);

        let response = self.client
            .put(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to register empty stream: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("go2rtc returned {}: {}", status, body));
        }

        log::info!("Registered empty stream '{}' with go2rtc (ready for push)", name);
        Ok(self.get_whep_url(name))
    }

    /// Get the HTTP MPEG-TS push URL for a stream
    pub fn get_ts_push_url(&self, name: &str) -> String {
        format!("{}/api/stream.ts?dst={}", self.base_url, name)
    }

    /// Unregister (remove) a stream from go2rtc
    pub async fn unregister_stream(&self, name: &str) -> Result<(), String> {
        let url = format!("{}/api/streams?src={}", self.base_url, name);

        let response = self.client
            .delete(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to unregister stream: {}", e))?;

        if !response.status().is_success() && response.status() != reqwest::StatusCode::NOT_FOUND {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("go2rtc returned {}: {}", status, body));
        }

        log::info!("Unregistered stream '{}' from go2rtc", name);
        Ok(())
    }

    /// Get the WHEP URL for WebRTC playback
    pub fn get_whep_url(&self, name: &str) -> String {
        format!("{}/api/webrtc?src={}", self.base_url, name)
    }

    /// Get the WebSocket URL for MSE/MJPEG playback
    pub fn get_ws_url(&self, name: &str) -> String {
        let ws_base = self.base_url.replace("http://", "ws://").replace("https://", "wss://");
        format!("{}/api/ws?src={}", ws_base, name)
    }

    /// Get WebRTC info for a stream
    pub fn get_webrtc_info(&self, name: &str) -> WebRtcInfo {
        WebRtcInfo {
            available: true,
            whep_url: Some(self.get_whep_url(name)),
            ws_url: Some(self.get_ws_url(name)),
            stream_name: Some(name.to_string()),
        }
    }

    /// List all registered streams
    pub async fn list_streams(&self) -> Result<Vec<String>, String> {
        let url = format!("{}/api/streams", self.base_url);

        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to list streams: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("go2rtc returned {}: {}", status, body));
        }

        let streams: StreamsResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse streams response: {}", e))?;

        Ok(streams.0.keys().cloned().collect())
    }

    /// Check if a stream is registered
    pub async fn is_stream_registered(&self, name: &str) -> bool {
        match self.list_streams().await {
            Ok(streams) => streams.contains(&name.to_string()),
            Err(_) => false,
        }
    }

    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

impl Default for Go2RtcClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a WebRtcInfo indicating go2rtc is unavailable
pub fn unavailable_webrtc_info() -> WebRtcInfo {
    WebRtcInfo {
        available: false,
        whep_url: None,
        ws_url: None,
        stream_name: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whep_url() {
        let client = Go2RtcClient::new();
        assert_eq!(
            client.get_whep_url("test-source"),
            "http://127.0.0.1:1984/api/webrtc?src=test-source"
        );
    }

    #[test]
    fn test_ws_url() {
        let client = Go2RtcClient::new();
        assert_eq!(
            client.get_ws_url("test-source"),
            "ws://127.0.0.1:1984/api/ws?src=test-source"
        );
    }
}
