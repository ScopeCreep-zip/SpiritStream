// Go2rtc Service
// Manages go2rtc process for WebRTC-based live preview

use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::models::Source;

// Windows: Hide console windows for spawned processes
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Default go2rtc API port
const DEFAULT_PORT: u16 = 1984;

/// Maximum concurrent WebRTC streams
const MAX_WEBRTC_STREAMS: usize = 5;

/// Orphan cleanup timeout (seconds)
const ORPHAN_TIMEOUT_SECS: u64 = 60;

/// Tracks an active WebRTC stream
#[derive(Debug)]
struct WebrtcStream {
    last_accessed: Instant,
}

/// Go2rtc service for managing WebRTC live preview
pub struct Go2rtcService {
    /// Path to go2rtc binary
    binary_path: String,
    /// go2rtc HTTP API port
    port: u16,
    /// go2rtc process handle (std Mutex - never held across await)
    process: StdMutex<Option<Child>>,
    /// Active WebRTC streams (stream_id -> WebrtcStream) - tokio Mutex for async safety
    streams: Mutex<HashMap<String, WebrtcStream>>,
    /// HTTP client for go2rtc API
    client: reqwest::Client,
}

impl Go2rtcService {
    /// Create a new Go2rtc service
    pub fn new(binary_path: String) -> Self {
        Self {
            binary_path,
            port: DEFAULT_PORT,
            process: StdMutex::new(None),
            streams: Mutex::new(HashMap::new()),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Set the go2rtc port (must be called before start)
    pub fn set_port(&mut self, port: u16) {
        self.port = port;
    }

    /// Get the go2rtc API base URL
    pub fn api_base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Get the WebSocket signaling URL for a stream
    pub fn get_webrtc_ws_url(&self, stream_id: &str) -> String {
        format!("ws://127.0.0.1:{}/api/ws?src={}", self.port, stream_id)
    }

    /// Check if go2rtc is available
    pub fn is_available(&self) -> bool {
        std::path::Path::new(&self.binary_path).exists()
    }

    /// Start the go2rtc process
    pub async fn start(&self) -> Result<(), String> {
        // Check if already running
        {
            let process = self.process.lock().map_err(|e| format!("Lock poisoned: {}", e))?;
            if process.is_some() {
                log::debug!("go2rtc already running");
                return Ok(());
            }
        }

        if !self.is_available() {
            return Err(format!("go2rtc binary not found at: {}", self.binary_path));
        }

        log::info!("Starting go2rtc on port {}...", self.port);

        // Start go2rtc with inline YAML config via -c flag
        // Format: -c "api: { listen: ':PORT' }"
        let config = format!(r#"api: {{ listen: ":{}" }}"#, self.port);

        let mut cmd = Command::new(&self.binary_path);
        cmd.arg("-c")
            .arg(&config)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let child = cmd.spawn()
            .map_err(|e| format!("Failed to start go2rtc: {}", e))?;

        log::info!("go2rtc process started with PID: {}", child.id());

        // Store the process handle
        {
            let mut process = self.process.lock().map_err(|e| format!("Lock poisoned: {}", e))?;
            *process = Some(child);
        }

        // Wait for API to be ready
        self.wait_for_ready().await?;

        Ok(())
    }

    /// Wait for go2rtc API to be ready
    async fn wait_for_ready(&self) -> Result<(), String> {
        let start = Instant::now();
        let timeout = Duration::from_secs(10);

        while start.elapsed() < timeout {
            match self.client.get(&format!("{}/api", self.api_base_url())).send().await {
                Ok(response) if response.status().is_success() => {
                    log::info!("go2rtc API ready");
                    return Ok(());
                }
                _ => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }

        Err("go2rtc API did not become ready in time".to_string())
    }

    /// Stop the go2rtc process
    pub fn stop(&self) {
        let mut process = match self.process.lock() {
            Ok(p) => p,
            Err(e) => {
                log::error!("Failed to lock go2rtc process: {}", e);
                return;
            }
        };

        if let Some(mut child) = process.take() {
            log::info!("Stopping go2rtc process...");
            let _ = child.kill();
            let _ = child.wait();
            log::info!("go2rtc process stopped");
        }

        // Clear all streams (use try_lock since this is sync)
        if let Ok(mut streams) = self.streams.try_lock() {
            streams.clear();
        }
    }

    /// Check if go2rtc is running
    pub fn is_running(&self) -> bool {
        match self.process.lock() {
            Ok(process) => process.is_some(),
            Err(_) => false,
        }
    }

    /// Build go2rtc source URL for a source
    /// Uses go2rtc's native ffmpeg: source format which handles device capture properly
    fn build_go2rtc_source(&self, source: &Source) -> Result<String, String> {
        match source {
            Source::Camera(cam) => {
                if cam.device_id.is_empty() {
                    return Err("Camera device not selected".to_string());
                }

                // go2rtc ffmpeg:device format: ffmpeg:device?video=INDEX&framerate=30#video=h264
                let mut params = vec![format!("video={}", cam.device_id)];

                if let Some(fps) = cam.fps {
                    params.push(format!("framerate={}", fps));
                }

                if let (Some(w), Some(h)) = (cam.width, cam.height) {
                    params.push(format!("video_size={}x{}", w, h));
                }

                // Use h264 encoding for WebRTC compatibility
                Ok(format!("ffmpeg:device?{}#video=h264#hardware", params.join("&")))
            }

            Source::ScreenCapture(screen) => {
                if screen.display_id.is_empty() {
                    return Err("Display not selected".to_string());
                }

                // Screen capture uses video device index (screens are listed after cameras)
                let mut params = vec![format!("video={}", screen.display_id)];
                params.push(format!("framerate={}", screen.fps));

                Ok(format!("ffmpeg:device?{}#video=h264", params.join("&")))
            }

            Source::MediaFile(media) => {
                if media.file_path.is_empty() {
                    return Err("Media file path not specified".to_string());
                }

                // For media files, use ffmpeg: with the file path
                // #video=copy to avoid re-encoding if possible
                Ok(format!("ffmpeg:{}#video=h264", media.file_path))
            }

            Source::CaptureCard(card) => {
                if card.device_id.is_empty() {
                    return Err("Capture card device not selected".to_string());
                }

                // Capture cards are also devices
                let params = format!("video={}", card.device_id);
                Ok(format!("ffmpeg:device?{}#video=h264#hardware", params))
            }

            Source::Rtmp(rtmp) => {
                // RTMP sources can be passed directly to go2rtc
                let host = if rtmp.bind_address == "0.0.0.0" {
                    "127.0.0.1"
                } else {
                    &rtmp.bind_address
                };
                let rtmp_url = format!("rtmp://{}:{}/{}", host, rtmp.port, rtmp.application);

                // go2rtc can handle RTMP directly
                Ok(rtmp_url)
            }

            Source::AudioDevice(_) => {
                // Audio-only sources - use a test pattern for video preview
                Ok("ffmpeg:lavfi?i=color=c=darkblue:s=320x180:d=3600#video=h264".to_string())
            }
        }
    }

    /// Start a WebRTC stream for a source
    pub async fn start_stream(&self, source: &Source) -> Result<String, String> {
        // Ensure go2rtc is running
        if !self.is_running() {
            self.start().await?;
        }

        let stream_id = format!("preview_{}", source.id());

        // Check for existing stream and handle eviction
        let evict_id: Option<String> = {
            let mut streams = self.streams.lock().await;

            if let Some(existing) = streams.get_mut(&stream_id) {
                existing.last_accessed = Instant::now();
                log::debug!("Reusing existing WebRTC stream: {}", stream_id);
                return Ok(stream_id);
            }

            // Enforce max streams (LRU eviction)
            if streams.len() >= MAX_WEBRTC_STREAMS {
                let oldest = streams.iter()
                    .min_by_key(|(_, s)| s.last_accessed)
                    .map(|(id, _)| id.clone());

                if let Some(ref oldest_id) = oldest {
                    log::info!("Evicting old WebRTC stream: {}", oldest_id);
                    streams.remove(oldest_id);
                }
                oldest
            } else {
                None
            }
        };

        // Remove evicted stream from go2rtc (outside the lock)
        if let Some(oldest_id) = evict_id {
            let _ = self.remove_stream_from_go2rtc(&oldest_id).await;
        }

        // Build go2rtc source URL
        let stream_source = self.build_go2rtc_source(source)?;

        log::info!("Adding WebRTC stream {}: {}", stream_id, stream_source);

        // Add stream to go2rtc via API
        self.add_stream_to_go2rtc(&stream_id, &stream_source).await?;

        // Track the stream
        {
            let mut streams = self.streams.lock().await;
            streams.insert(stream_id.clone(), WebrtcStream {
                last_accessed: Instant::now(),
            });
        }

        Ok(stream_id)
    }

    /// Add a stream to go2rtc via its HTTP API
    async fn add_stream_to_go2rtc(&self, stream_id: &str, source: &str) -> Result<(), String> {
        let url = format!("{}/api/streams?src={}", self.api_base_url(), stream_id);

        let response = self.client
            .put(&url)
            .body(source.to_string())
            .send()
            .await
            .map_err(|e| format!("Failed to add stream to go2rtc: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("go2rtc API error: {} - {}", status, body));
        }

        Ok(())
    }

    /// Remove a stream from go2rtc via its HTTP API
    async fn remove_stream_from_go2rtc(&self, stream_id: &str) -> Result<(), String> {
        let url = format!("{}/api/streams?src={}", self.api_base_url(), stream_id);

        let response = self.client
            .delete(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to remove stream from go2rtc: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("go2rtc API error: {} - {}", status, body));
        }

        Ok(())
    }

    /// Stop a WebRTC stream
    pub async fn stop_stream(&self, source_id: &str) -> Result<(), String> {
        let stream_id = format!("preview_{}", source_id);

        // Remove from tracking
        {
            let mut streams = self.streams.lock().await;
            streams.remove(&stream_id);
        }

        // Remove from go2rtc
        self.remove_stream_from_go2rtc(&stream_id).await?;

        log::info!("Stopped WebRTC stream: {}", stream_id);
        Ok(())
    }

    /// Stop all WebRTC streams
    pub async fn stop_all_streams(&self) {
        let stream_ids: Vec<String> = {
            let mut streams = self.streams.lock().await;
            let ids: Vec<String> = streams.keys().cloned().collect();
            streams.clear();
            ids
        };

        for stream_id in stream_ids {
            let _ = self.remove_stream_from_go2rtc(&stream_id).await;
        }
    }

    /// Get info about an active stream
    pub async fn get_stream_info(&self, source_id: &str) -> Option<(String, String)> {
        let stream_id = format!("preview_{}", source_id);

        let mut streams = self.streams.lock().await;
        if let Some(stream) = streams.get_mut(&stream_id) {
            stream.last_accessed = Instant::now();
            Some((stream_id.clone(), self.get_webrtc_ws_url(&stream_id)))
        } else {
            None
        }
    }

    /// Cleanup orphaned streams (not accessed in ORPHAN_TIMEOUT_SECS)
    pub async fn cleanup_orphaned_streams(&self) {
        let timeout = Duration::from_secs(ORPHAN_TIMEOUT_SECS);
        let now = Instant::now();

        let orphans: Vec<String> = {
            let streams = self.streams.lock().await;

            streams.iter()
                .filter(|(_, s)| now.duration_since(s.last_accessed) > timeout)
                .map(|(id, _)| id.clone())
                .collect()
        };

        for stream_id in orphans {
            log::info!("Cleaning up orphaned WebRTC stream: {}", stream_id);
            {
                let mut streams = self.streams.lock().await;
                streams.remove(&stream_id);
            }
            let _ = self.remove_stream_from_go2rtc(&stream_id).await;
        }
    }

    /// Get count of active streams
    pub fn active_stream_count(&self) -> usize {
        self.streams.try_lock().map(|s| s.len()).unwrap_or(0)
    }
}

impl Drop for Go2rtcService {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webrtc_url() {
        let service = Go2rtcService::new("/path/to/go2rtc".to_string(), "/path/to/ffmpeg".to_string());
        let url = service.get_webrtc_ws_url("preview_test");
        assert_eq!(url, "ws://127.0.0.1:1984/api/ws?src=preview_test");
    }

    #[test]
    fn test_api_base_url() {
        let service = Go2rtcService::new("/path/to/go2rtc".to_string(), "/path/to/ffmpeg".to_string());
        assert_eq!(service.api_base_url(), "http://127.0.0.1:1984");
    }
}
