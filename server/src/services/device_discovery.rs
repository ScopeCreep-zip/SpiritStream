// Device Discovery Service
// Platform-specific device enumeration for cameras, displays, audio devices, and capture cards
// With async support and caching to prevent blocking the tokio runtime

use crate::models::{CameraDevice, DisplayInfo, AudioInputDevice, CaptureCardDevice, Resolution};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::timeout;

/// Cache TTL in seconds (30 seconds)
const CACHE_TTL_SECS: u64 = 30;

/// Timeout for each device enumeration command (15 seconds)
/// Increased from 5s to handle systems with many devices or slow USB enumeration
const ENUMERATION_TIMEOUT_SECS: u64 = 15;

/// Maximum retry attempts for device enumeration
const MAX_RETRY_ATTEMPTS: u32 = 3;

/// Base delay for exponential backoff (milliseconds)
const RETRY_BASE_DELAY_MS: u64 = 1000;

/// Execute an async operation with exponential backoff retry
async fn retry_with_backoff<T, F, Fut>(
    operation_name: &str,
    mut operation: F,
) -> Option<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    for attempt in 0..MAX_RETRY_ATTEMPTS {
        if attempt > 0 {
            // Exponential backoff: 1s, 2s, 4s...
            let delay_ms = RETRY_BASE_DELAY_MS * (1 << (attempt - 1));
            log::debug!(
                "[DeviceDiscovery] Retry {} for {}, waiting {}ms",
                attempt,
                operation_name,
                delay_ms
            );
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }

        if let Some(result) = operation().await {
            return Some(result);
        }

        log::warn!(
            "[DeviceDiscovery] {} attempt {} failed",
            operation_name,
            attempt + 1
        );
    }

    log::error!(
        "[DeviceDiscovery] {} failed after {} attempts",
        operation_name,
        MAX_RETRY_ATTEMPTS
    );
    None
}

/// Cached device data with timestamp
#[derive(Debug, Clone, Default)]
pub struct CachedDevices {
    pub cameras: Vec<CameraDevice>,
    pub displays: Vec<DisplayInfo>,
    pub audio_devices: Vec<AudioInputDevice>,
    pub capture_cards: Vec<CaptureCardDevice>,
    pub last_update: Option<Instant>,
}

impl CachedDevices {
    /// Check if the cache is still valid (within TTL)
    pub fn is_valid(&self) -> bool {
        self.last_update
            .map(|t| t.elapsed() < Duration::from_secs(CACHE_TTL_SECS))
            .unwrap_or(false)
    }
}

/// Thread-safe device cache
pub struct DeviceCache {
    devices: RwLock<CachedDevices>,
}

impl DeviceCache {
    pub fn new() -> Self {
        Self {
            devices: RwLock::new(CachedDevices::default()),
        }
    }

    /// Get cached devices if still valid
    pub async fn get(&self) -> Option<CachedDevices> {
        let cache = self.devices.read().await;
        if cache.is_valid() {
            Some(cache.clone())
        } else {
            None
        }
    }

    /// Update the cache with new device data
    pub async fn update(&self, devices: CachedDevices) {
        let mut cache = self.devices.write().await;
        *cache = CachedDevices {
            last_update: Some(Instant::now()),
            ..devices
        };
    }

    /// Get cameras from cache (even if expired, for fallback)
    pub async fn get_cameras(&self) -> Vec<CameraDevice> {
        self.devices.read().await.cameras.clone()
    }

    /// Get displays from cache (even if expired, for fallback)
    pub async fn get_displays(&self) -> Vec<DisplayInfo> {
        self.devices.read().await.displays.clone()
    }

    /// Get audio devices from cache (even if expired, for fallback)
    pub async fn get_audio_devices(&self) -> Vec<AudioInputDevice> {
        self.devices.read().await.audio_devices.clone()
    }

    /// Get capture cards from cache (even if expired, for fallback)
    pub async fn get_capture_cards(&self) -> Vec<CaptureCardDevice> {
        self.devices.read().await.capture_cards.clone()
    }
}

impl Default for DeviceCache {
    fn default() -> Self {
        Self::new()
    }
}

/// All devices result for refresh_devices_async
#[derive(Debug, Clone, Default)]
pub struct AllDevices {
    pub cameras: Vec<CameraDevice>,
    pub displays: Vec<DisplayInfo>,
    pub audio_devices: Vec<AudioInputDevice>,
    pub capture_cards: Vec<CaptureCardDevice>,
}

/// Device discovery service for enumerating available input devices
/// Provides both sync (blocking) and async (non-blocking) methods
pub struct DeviceDiscovery {
    ffmpeg_path: String,
    cache: Arc<DeviceCache>,
}

impl DeviceDiscovery {
    /// Create a new DeviceDiscovery instance with the specified FFmpeg path
    pub fn new(ffmpeg_path: String) -> Self {
        Self {
            ffmpeg_path,
            cache: Arc::new(DeviceCache::new()),
        }
    }

    /// Create a new DeviceDiscovery with a shared cache
    pub fn with_cache(ffmpeg_path: String, cache: Arc<DeviceCache>) -> Self {
        Self { ffmpeg_path, cache }
    }

    /// Get a reference to the cache for sharing between instances
    pub fn cache(&self) -> Arc<DeviceCache> {
        self.cache.clone()
    }

    // ============================================================
    // Async methods (non-blocking, with caching)
    // ============================================================

    /// List all devices asynchronously with caching
    /// Returns cached data immediately if valid, refreshes in background if stale
    pub async fn refresh_devices_async(&self) -> Result<AllDevices, String> {
        // Check cache first
        if let Some(cached) = self.cache.get().await {
            log::debug!("[DeviceDiscovery] Cache hit, returning cached devices");
            return Ok(AllDevices {
                cameras: cached.cameras,
                displays: cached.displays,
                audio_devices: cached.audio_devices,
                capture_cards: cached.capture_cards,
            });
        }

        log::info!("[DeviceDiscovery] Cache miss, fetching devices asynchronously");

        // Run all enumerations in parallel using spawn_blocking
        let ffmpeg_path = self.ffmpeg_path.clone();
        let ffmpeg_path2 = self.ffmpeg_path.clone();
        let ffmpeg_path3 = self.ffmpeg_path.clone();
        let ffmpeg_path4 = self.ffmpeg_path.clone();

        // Spawn blocking tasks in parallel
        let cameras_handle = tokio::spawn(async move {
            timeout(
                Duration::from_secs(ENUMERATION_TIMEOUT_SECS),
                tokio::task::spawn_blocking(move || {
                    Self::list_cameras_sync(&ffmpeg_path)
                })
            ).await
        });

        let displays_handle = tokio::spawn(async move {
            timeout(
                Duration::from_secs(ENUMERATION_TIMEOUT_SECS),
                tokio::task::spawn_blocking(move || {
                    Self::list_displays_sync(&ffmpeg_path2)
                })
            ).await
        });

        let audio_handle = tokio::spawn(async move {
            timeout(
                Duration::from_secs(ENUMERATION_TIMEOUT_SECS),
                tokio::task::spawn_blocking(move || {
                    Self::list_audio_inputs_sync(&ffmpeg_path3)
                })
            ).await
        });

        let capture_cards_handle = tokio::spawn(async move {
            timeout(
                Duration::from_secs(ENUMERATION_TIMEOUT_SECS),
                tokio::task::spawn_blocking(move || {
                    Self::list_capture_cards_sync(&ffmpeg_path4)
                })
            ).await
        });

        // Await all results
        let (cameras_result, displays_result, audio_result, cards_result) = tokio::join!(
            cameras_handle,
            displays_handle,
            audio_handle,
            capture_cards_handle
        );

        // Extract results with fallbacks for timeouts/errors
        let cameras = cameras_result
            .ok()
            .and_then(|r| r.ok())
            .and_then(|r| r.ok())
            .and_then(|r| r.ok())
            .unwrap_or_else(|| {
                log::warn!("[DeviceDiscovery] Camera enumeration failed or timed out");
                Vec::new()
            });

        let displays = displays_result
            .ok()
            .and_then(|r| r.ok())
            .and_then(|r| r.ok())
            .and_then(|r| r.ok())
            .unwrap_or_else(|| {
                log::warn!("[DeviceDiscovery] Display enumeration failed or timed out");
                Vec::new()
            });

        let audio_devices = audio_result
            .ok()
            .and_then(|r| r.ok())
            .and_then(|r| r.ok())
            .and_then(|r| r.ok())
            .unwrap_or_else(|| {
                log::warn!("[DeviceDiscovery] Audio enumeration failed or timed out");
                Vec::new()
            });

        let capture_cards = cards_result
            .ok()
            .and_then(|r| r.ok())
            .and_then(|r| r.ok())
            .and_then(|r| r.ok())
            .unwrap_or_else(|| {
                log::warn!("[DeviceDiscovery] Capture card enumeration failed or timed out");
                Vec::new()
            });

        // Update cache
        self.cache.update(CachedDevices {
            cameras: cameras.clone(),
            displays: displays.clone(),
            audio_devices: audio_devices.clone(),
            capture_cards: capture_cards.clone(),
            last_update: Some(Instant::now()),
        }).await;

        log::info!(
            "[DeviceDiscovery] Refresh complete: {} cameras, {} displays, {} audio, {} capture cards",
            cameras.len(), displays.len(), audio_devices.len(), capture_cards.len()
        );

        Ok(AllDevices {
            cameras,
            displays,
            audio_devices,
            capture_cards,
        })
    }

    /// List cameras asynchronously with retry
    pub async fn list_cameras_async(&self) -> Result<Vec<CameraDevice>, String> {
        // Check cache first
        if let Some(cached) = self.cache.get().await {
            return Ok(cached.cameras);
        }

        let ffmpeg_path = self.ffmpeg_path.clone();
        let cache = self.cache.clone();

        // Use retry with exponential backoff
        let result = retry_with_backoff("camera enumeration", || {
            let path = ffmpeg_path.clone();
            async move {
                let res = timeout(
                    Duration::from_secs(ENUMERATION_TIMEOUT_SECS),
                    tokio::task::spawn_blocking(move || Self::list_cameras_sync(&path)),
                )
                .await;

                match res {
                    Ok(Ok(Ok(cameras))) => Some(cameras),
                    _ => None,
                }
            }
        })
        .await;

        match result {
            Some(cameras) => Ok(cameras),
            None => {
                log::warn!("[DeviceDiscovery] Camera enumeration failed after retries, returning cached data");
                Ok(cache.get_cameras().await)
            }
        }
    }

    /// List displays asynchronously with retry
    pub async fn list_displays_async(&self) -> Result<Vec<DisplayInfo>, String> {
        if let Some(cached) = self.cache.get().await {
            return Ok(cached.displays);
        }

        let ffmpeg_path = self.ffmpeg_path.clone();
        let cache = self.cache.clone();

        let result = retry_with_backoff("display enumeration", || {
            let path = ffmpeg_path.clone();
            async move {
                let res = timeout(
                    Duration::from_secs(ENUMERATION_TIMEOUT_SECS),
                    tokio::task::spawn_blocking(move || Self::list_displays_sync(&path)),
                )
                .await;

                match res {
                    Ok(Ok(Ok(displays))) => Some(displays),
                    _ => None,
                }
            }
        })
        .await;

        match result {
            Some(displays) => Ok(displays),
            None => {
                log::warn!("[DeviceDiscovery] Display enumeration failed after retries, returning cached data");
                Ok(cache.get_displays().await)
            }
        }
    }

    /// List audio inputs asynchronously with retry
    pub async fn list_audio_inputs_async(&self) -> Result<Vec<AudioInputDevice>, String> {
        if let Some(cached) = self.cache.get().await {
            return Ok(cached.audio_devices);
        }

        let ffmpeg_path = self.ffmpeg_path.clone();
        let cache = self.cache.clone();

        let result = retry_with_backoff("audio input enumeration", || {
            let path = ffmpeg_path.clone();
            async move {
                let res = timeout(
                    Duration::from_secs(ENUMERATION_TIMEOUT_SECS),
                    tokio::task::spawn_blocking(move || Self::list_audio_inputs_sync(&path)),
                )
                .await;

                match res {
                    Ok(Ok(Ok(devices))) => Some(devices),
                    _ => None,
                }
            }
        })
        .await;

        match result {
            Some(devices) => Ok(devices),
            None => {
                log::warn!("[DeviceDiscovery] Audio enumeration failed after retries, returning cached data");
                Ok(cache.get_audio_devices().await)
            }
        }
    }

    /// List capture cards asynchronously with retry
    pub async fn list_capture_cards_async(&self) -> Result<Vec<CaptureCardDevice>, String> {
        if let Some(cached) = self.cache.get().await {
            return Ok(cached.capture_cards);
        }

        let ffmpeg_path = self.ffmpeg_path.clone();
        let cache = self.cache.clone();

        let result = retry_with_backoff("capture card enumeration", || {
            let path = ffmpeg_path.clone();
            async move {
                let res = timeout(
                    Duration::from_secs(ENUMERATION_TIMEOUT_SECS),
                    tokio::task::spawn_blocking(move || Self::list_capture_cards_sync(&path)),
                )
                .await;

                match res {
                    Ok(Ok(Ok(cards))) => Some(cards),
                    _ => None,
                }
            }
        })
        .await;

        match result {
            Some(cards) => Ok(cards),
            None => {
                log::warn!("[DeviceDiscovery] Capture card enumeration failed after retries, returning cached data");
                Ok(cache.get_capture_cards().await)
            }
        }
    }

    // ============================================================
    // Sync methods (blocking) - kept for backwards compatibility
    // ============================================================

    /// List available camera devices (blocking)
    pub fn list_cameras(&self) -> Result<Vec<CameraDevice>, String> {
        Self::list_cameras_sync(&self.ffmpeg_path)
    }

    /// List available displays (blocking)
    pub fn list_displays(&self) -> Result<Vec<DisplayInfo>, String> {
        Self::list_displays_sync(&self.ffmpeg_path)
    }

    /// List available audio input devices (blocking)
    pub fn list_audio_inputs(&self) -> Result<Vec<AudioInputDevice>, String> {
        Self::list_audio_inputs_sync(&self.ffmpeg_path)
    }

    /// List available capture cards (blocking)
    pub fn list_capture_cards(&self) -> Result<Vec<CaptureCardDevice>, String> {
        Self::list_capture_cards_sync(&self.ffmpeg_path)
    }

    // ============================================================
    // Static sync implementations
    // ============================================================

    fn list_cameras_sync(ffmpeg_path: &str) -> Result<Vec<CameraDevice>, String> {
        #[cfg(target_os = "macos")]
        {
            Self::list_cameras_macos(ffmpeg_path)
        }
        #[cfg(target_os = "windows")]
        {
            Self::list_cameras_windows(ffmpeg_path)
        }
        #[cfg(target_os = "linux")]
        {
            Self::list_cameras_linux(ffmpeg_path)
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = ffmpeg_path;
            Err("Camera discovery not supported on this platform".to_string())
        }
    }

    fn list_displays_sync(ffmpeg_path: &str) -> Result<Vec<DisplayInfo>, String> {
        #[cfg(target_os = "macos")]
        {
            Self::list_displays_macos(ffmpeg_path)
        }
        #[cfg(target_os = "windows")]
        {
            Self::list_displays_windows(ffmpeg_path)
        }
        #[cfg(target_os = "linux")]
        {
            Self::list_displays_linux(ffmpeg_path)
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = ffmpeg_path;
            Err("Display discovery not supported on this platform".to_string())
        }
    }

    fn list_audio_inputs_sync(ffmpeg_path: &str) -> Result<Vec<AudioInputDevice>, String> {
        #[cfg(target_os = "macos")]
        {
            Self::list_audio_inputs_macos(ffmpeg_path)
        }
        #[cfg(target_os = "windows")]
        {
            Self::list_audio_inputs_windows(ffmpeg_path)
        }
        #[cfg(target_os = "linux")]
        {
            Self::list_audio_inputs_linux(ffmpeg_path)
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = ffmpeg_path;
            Err("Audio device discovery not supported on this platform".to_string())
        }
    }

    fn list_capture_cards_sync(ffmpeg_path: &str) -> Result<Vec<CaptureCardDevice>, String> {
        #[cfg(target_os = "macos")]
        {
            Self::list_capture_cards_macos(ffmpeg_path)
        }
        #[cfg(target_os = "windows")]
        {
            Self::list_capture_cards_windows(ffmpeg_path)
        }
        #[cfg(target_os = "linux")]
        {
            Self::list_capture_cards_linux(ffmpeg_path)
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = ffmpeg_path;
            Err("Capture card discovery not supported on this platform".to_string())
        }
    }

    // ============================================================
    // macOS implementations using AVFoundation via FFmpeg
    // ============================================================

    #[cfg(target_os = "macos")]
    fn list_cameras_macos(ffmpeg_path: &str) -> Result<Vec<CameraDevice>, String> {
        // Use FFmpeg to list AVFoundation devices
        let output = Command::new(ffmpeg_path)
            .args(["-f", "avfoundation", "-list_devices", "true", "-i", ""])
            .output()
            .map_err(|e| format!("Failed to run FFmpeg: {}", e))?;

        // FFmpeg writes device list to stderr
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut cameras = Self::parse_avfoundation_cameras(&stderr)?;

        // Get audio devices for auto-pairing
        let audio_devices = Self::parse_avfoundation_audio(&stderr)?;

        // Auto-pair cameras with their microphones
        Self::pair_cameras_with_audio(&mut cameras, &audio_devices);

        Ok(cameras)
    }

    #[cfg(target_os = "macos")]
    fn parse_avfoundation_cameras(output: &str) -> Result<Vec<CameraDevice>, String> {
        let mut cameras = Vec::new();
        let mut in_video_section = false;

        for line in output.lines() {
            if line.contains("AVFoundation video devices:") {
                in_video_section = true;
                continue;
            }
            if line.contains("AVFoundation audio devices:") {
                break;
            }
            if in_video_section {
                // Parse lines like "[AVFoundation indev @ 0x...] [0] FaceTime HD Camera"
                if let Some(bracket_pos) = line.find("] [") {
                    let rest = &line[bracket_pos + 3..];
                    if let Some(end_bracket) = rest.find(']') {
                        if let Ok(idx) = rest[..end_bracket].parse::<usize>() {
                            let name = rest[end_bracket + 2..].trim().to_string();
                            // Skip screen capture devices (they show as video but are displays)
                            if !name.contains("Capture screen") && !name.is_empty() {
                                cameras.push(CameraDevice {
                                    device_id: idx.to_string(),
                                    name,
                                    resolutions: vec![
                                        Resolution { width: 1920, height: 1080, fps: vec![30, 60] },
                                        Resolution { width: 1280, height: 720, fps: vec![30, 60] },
                                        Resolution { width: 640, height: 480, fps: vec![30] },
                                    ],
                                    linked_audio_device_id: None,
                                    linked_audio_device_name: None,
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(cameras)
    }

    /// Auto-pair cameras with their linked audio devices by name matching
    /// Uses keyword matching to find related audio devices (e.g., FaceTime Camera -> Built-in Microphone)
    fn pair_cameras_with_audio(
        cameras: &mut Vec<CameraDevice>,
        audio_devices: &[AudioInputDevice],
    ) {
        for camera in cameras.iter_mut() {
            let camera_name_lower = camera.name.to_lowercase();

            // Extract keywords from camera name (skip common generic terms)
            let keywords: Vec<&str> = camera_name_lower
                .split(|c: char| c.is_whitespace() || c == '-' || c == '(' || c == ')')
                .filter(|s| s.len() > 3 && *s != "camera" && *s != "webcam" && *s != "video")
                .collect();

            for audio in audio_devices {
                let audio_name_lower = audio.name.to_lowercase();

                // Strategy 1: Match by shared keywords (e.g., "FaceTime", "Logitech", "C920")
                let keyword_match = keywords.iter().any(|kw| audio_name_lower.contains(kw));

                // Strategy 2: Special case for macOS - FaceTime camera -> Built-in Microphone
                let builtin_match = camera_name_lower.contains("facetime")
                    && (audio_name_lower.contains("built-in") || audio_name_lower.contains("macbook"));

                // Strategy 3: USB cameras often have matching names (e.g., "Logitech C920" camera and audio)
                let exact_prefix_match = !camera_name_lower.contains("facetime")
                    && audio_name_lower.starts_with(&camera_name_lower[..camera_name_lower.len().min(10)]);

                if keyword_match || builtin_match || exact_prefix_match {
                    camera.linked_audio_device_id = Some(audio.device_id.clone());
                    camera.linked_audio_device_name = Some(audio.name.clone());
                    break;
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn list_displays_macos(ffmpeg_path: &str) -> Result<Vec<DisplayInfo>, String> {
        // Use FFmpeg to list screen capture devices
        let output = Command::new(ffmpeg_path)
            .args(["-f", "avfoundation", "-list_devices", "true", "-i", ""])
            .output()
            .map_err(|e| format!("Failed to run FFmpeg: {}", e))?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        Self::parse_avfoundation_displays(&stderr)
    }

    #[cfg(target_os = "macos")]
    fn parse_avfoundation_displays(output: &str) -> Result<Vec<DisplayInfo>, String> {
        let mut displays = Vec::new();
        let mut in_video_section = false;

        for line in output.lines() {
            if line.contains("AVFoundation video devices:") {
                in_video_section = true;
                continue;
            }
            if line.contains("AVFoundation audio devices:") {
                break;
            }
            if in_video_section {
                if let Some(bracket_pos) = line.find("] [") {
                    let rest = &line[bracket_pos + 3..];
                    if let Some(end_bracket) = rest.find(']') {
                        if let Ok(idx) = rest[..end_bracket].parse::<usize>() {
                            let name = rest[end_bracket + 2..].trim().to_string();
                            // Only include screen capture devices
                            if name.contains("Capture screen") {
                                displays.push(DisplayInfo {
                                    display_id: idx.to_string(),
                                    name: format!("Display {}", displays.len() + 1),
                                    // Store the actual AVFoundation device name for go2rtc
                                    device_name: Some(name),
                                    width: 1920,  // Default, actual resolution needs screen query
                                    height: 1080,
                                    is_primary: displays.is_empty(),
                                });
                            }
                        }
                    }
                }
            }
        }

        // If no displays found via FFmpeg, add a default one
        if displays.is_empty() {
            displays.push(DisplayInfo {
                display_id: "0".to_string(),
                name: "Main Display".to_string(),
                device_name: Some("Capture screen 0".to_string()),
                width: 1920,
                height: 1080,
                is_primary: true,
            });
        }

        Ok(displays)
    }

    #[cfg(target_os = "macos")]
    fn list_audio_inputs_macos(ffmpeg_path: &str) -> Result<Vec<AudioInputDevice>, String> {
        let output = Command::new(ffmpeg_path)
            .args(["-f", "avfoundation", "-list_devices", "true", "-i", ""])
            .output()
            .map_err(|e| format!("Failed to run FFmpeg: {}", e))?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        Self::parse_avfoundation_audio(&stderr)
    }

    #[cfg(target_os = "macos")]
    fn parse_avfoundation_audio(output: &str) -> Result<Vec<AudioInputDevice>, String> {
        let mut devices = Vec::new();
        let mut in_audio_section = false;

        for line in output.lines() {
            if line.contains("AVFoundation audio devices:") {
                in_audio_section = true;
                continue;
            }
            if in_audio_section {
                if let Some(bracket_pos) = line.find("] [") {
                    let rest = &line[bracket_pos + 3..];
                    if let Some(end_bracket) = rest.find(']') {
                        if let Ok(idx) = rest[..end_bracket].parse::<usize>() {
                            let name = rest[end_bracket + 2..].trim().to_string();
                            if !name.is_empty() {
                                devices.push(AudioInputDevice {
                                    device_id: idx.to_string(),
                                    name,
                                    channels: 2,
                                    sample_rate: 48000,
                                    is_default: devices.is_empty(),
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(devices)
    }

    #[cfg(target_os = "macos")]
    fn list_capture_cards_macos(ffmpeg_path: &str) -> Result<Vec<CaptureCardDevice>, String> {
        // On macOS, capture cards appear as video devices in AVFoundation
        // We look for known capture card names
        let cameras = Self::list_cameras_macos(ffmpeg_path)?;
        let capture_card_keywords = ["elgato", "capture", "cam link", "game capture", "avermedia"];

        let capture_cards = cameras
            .into_iter()
            .filter(|c| {
                let name_lower = c.name.to_lowercase();
                capture_card_keywords.iter().any(|k| name_lower.contains(k))
            })
            .map(|c| CaptureCardDevice {
                device_id: c.device_id,
                name: c.name,
                inputs: vec!["hdmi".to_string()],
            })
            .collect();

        Ok(capture_cards)
    }

    // ============================================================
    // Windows implementations using DirectShow
    // ============================================================

    #[cfg(target_os = "windows")]
    fn list_cameras_windows(ffmpeg_path: &str) -> Result<Vec<CameraDevice>, String> {
        let output = Command::new(ffmpeg_path)
            .args(["-f", "dshow", "-list_devices", "true", "-i", "dummy"])
            .output()
            .map_err(|e| format!("Failed to run FFmpeg: {}", e))?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut cameras: Vec<CameraDevice> = Self::parse_dshow_devices(&stderr, "video")?
            .into_iter()
            .map(|(id, name)| CameraDevice {
                device_id: id,
                name,
                resolutions: vec![
                    Resolution { width: 1920, height: 1080, fps: vec![30, 60] },
                    Resolution { width: 1280, height: 720, fps: vec![30, 60] },
                ],
                linked_audio_device_id: None,
                linked_audio_device_name: None,
            })
            .collect();

        // Get audio devices for auto-pairing
        let audio_devices = Self::list_audio_inputs_windows(ffmpeg_path)?;
        Self::pair_cameras_with_audio(&mut cameras, &audio_devices);

        Ok(cameras)
    }

    #[cfg(target_os = "windows")]
    fn list_displays_windows(_ffmpeg_path: &str) -> Result<Vec<DisplayInfo>, String> {
        // Windows uses gdigrab for screen capture
        // List available monitors
        Ok(vec![DisplayInfo {
            display_id: "desktop".to_string(),
            name: "Primary Display".to_string(),
            device_name: None,
            width: 1920,
            height: 1080,
            is_primary: true,
        }])
    }

    #[cfg(target_os = "windows")]
    fn list_audio_inputs_windows(ffmpeg_path: &str) -> Result<Vec<AudioInputDevice>, String> {
        let output = Command::new(ffmpeg_path)
            .args(["-f", "dshow", "-list_devices", "true", "-i", "dummy"])
            .output()
            .map_err(|e| format!("Failed to run FFmpeg: {}", e))?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        Self::parse_dshow_devices(&stderr, "audio")
            .map(|names| {
                names.into_iter().enumerate().map(|(i, (id, name))| AudioInputDevice {
                    device_id: id,
                    name,
                    channels: 2,
                    sample_rate: 48000,
                    is_default: i == 0,
                }).collect()
            })
    }

    #[cfg(target_os = "windows")]
    fn list_capture_cards_windows(ffmpeg_path: &str) -> Result<Vec<CaptureCardDevice>, String> {
        let cameras = Self::list_cameras_windows(ffmpeg_path)?;
        let capture_card_keywords = ["elgato", "capture", "cam link", "game capture", "avermedia"];

        let capture_cards = cameras
            .into_iter()
            .filter(|c| {
                let name_lower = c.name.to_lowercase();
                capture_card_keywords.iter().any(|k| name_lower.contains(k))
            })
            .map(|c| CaptureCardDevice {
                device_id: c.device_id,
                name: c.name,
                inputs: vec!["hdmi".to_string()],
            })
            .collect();

        Ok(capture_cards)
    }

    #[cfg(target_os = "windows")]
    fn parse_dshow_devices(output: &str, device_type: &str) -> Result<Vec<(String, String)>, String> {
        let mut devices = Vec::new();
        let mut in_section = false;
        let section_marker = format!("DirectShow {} devices", device_type);

        for line in output.lines() {
            if line.contains(&section_marker) {
                in_section = true;
                continue;
            }
            if in_section && line.contains("DirectShow") && !line.contains(&section_marker) {
                break;
            }
            if in_section && line.contains("]  \"") {
                // Parse lines like '[dshow @ ...] "Device Name"'
                if let Some(start) = line.find('"') {
                    if let Some(end) = line[start+1..].find('"') {
                        let name = line[start+1..start+1+end].to_string();
                        devices.push((name.clone(), name));
                    }
                }
            }
        }

        Ok(devices)
    }

    // ============================================================
    // Linux implementations using V4L2
    // ============================================================

    #[cfg(target_os = "linux")]
    fn list_cameras_linux(ffmpeg_path: &str) -> Result<Vec<CameraDevice>, String> {
        let output = Command::new("v4l2-ctl")
            .args(["--list-devices"])
            .output()
            .map_err(|_| "v4l2-ctl not found. Install v4l-utils package.".to_string())?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut cameras = Self::parse_v4l2_cameras(&stdout)?;

        // Get audio devices for auto-pairing
        let audio_devices = Self::list_audio_inputs_linux(ffmpeg_path)?;
        Self::pair_cameras_with_audio(&mut cameras, &audio_devices);

        Ok(cameras)
    }

    #[cfg(target_os = "linux")]
    fn parse_v4l2_cameras(output: &str) -> Result<Vec<CameraDevice>, String> {
        let mut cameras = Vec::new();
        let mut current_name: Option<String> = None;

        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.ends_with(':') {
                // Device name line
                current_name = Some(trimmed.trim_end_matches(':').to_string());
            } else if trimmed.starts_with("/dev/video") && !trimmed.contains("1") {
                // Only use the first video device for each camera
                if let Some(ref name) = current_name {
                    cameras.push(CameraDevice {
                        device_id: trimmed.to_string(),
                        name: name.clone(),
                        resolutions: vec![
                            Resolution { width: 1920, height: 1080, fps: vec![30] },
                            Resolution { width: 1280, height: 720, fps: vec![30] },
                        ],
                        linked_audio_device_id: None,
                        linked_audio_device_name: None,
                    });
                }
                current_name = None;
            }
        }

        Ok(cameras)
    }

    #[cfg(target_os = "linux")]
    fn list_displays_linux(_ffmpeg_path: &str) -> Result<Vec<DisplayInfo>, String> {
        // Use xrandr to list displays
        let output = Command::new("xrandr")
            .output()
            .map_err(|_| "xrandr not found".to_string())?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut displays = Vec::new();

        for line in stdout.lines() {
            if line.contains(" connected") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(name) = parts.first() {
                    let is_primary = line.contains("primary");

                    // Try to parse resolution
                    let (width, height) = parts.iter()
                        .find(|p| p.contains('x') && p.chars().next().map_or(false, |c| c.is_ascii_digit()))
                        .and_then(|res| {
                            let dims: Vec<&str> = res.split(|c| c == 'x' || c == '+').collect();
                            if dims.len() >= 2 {
                                Some((
                                    dims[0].parse().unwrap_or(1920),
                                    dims[1].parse().unwrap_or(1080),
                                ))
                            } else {
                                None
                            }
                        })
                        .unwrap_or((1920, 1080));

                    displays.push(DisplayInfo {
                        display_id: name.to_string(),
                        name: name.to_string(),
                        device_name: None,
                        width,
                        height,
                        is_primary,
                    });
                }
            }
        }

        if displays.is_empty() {
            displays.push(DisplayInfo {
                display_id: ":0".to_string(),
                name: "Display :0".to_string(),
                device_name: None,
                width: 1920,
                height: 1080,
                is_primary: true,
            });
        }

        Ok(displays)
    }

    #[cfg(target_os = "linux")]
    fn list_audio_inputs_linux(_ffmpeg_path: &str) -> Result<Vec<AudioInputDevice>, String> {
        // Use pactl to list PulseAudio sources
        let output = Command::new("pactl")
            .args(["list", "sources", "short"])
            .output()
            .map_err(|_| "pactl not found. Install pulseaudio-utils package.".to_string())?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut devices = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let device_id = parts[1].to_string();
                // Skip monitor sources (they're output monitors, not inputs)
                if !device_id.contains(".monitor") {
                    devices.push(AudioInputDevice {
                        device_id: device_id.clone(),
                        name: device_id,
                        channels: 2,
                        sample_rate: 48000,
                        is_default: devices.is_empty(),
                    });
                }
            }
        }

        Ok(devices)
    }

    #[cfg(target_os = "linux")]
    fn list_capture_cards_linux(ffmpeg_path: &str) -> Result<Vec<CaptureCardDevice>, String> {
        let cameras = Self::list_cameras_linux(ffmpeg_path)?;
        let capture_card_keywords = ["elgato", "capture", "cam link", "game capture", "avermedia"];

        let capture_cards = cameras
            .into_iter()
            .filter(|c| {
                let name_lower = c.name.to_lowercase();
                capture_card_keywords.iter().any(|k| name_lower.contains(k))
            })
            .map(|c| CaptureCardDevice {
                device_id: c.device_id,
                name: c.name,
                inputs: vec!["hdmi".to_string()],
            })
            .collect();

        Ok(capture_cards)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_discovery() -> DeviceDiscovery {
        // Use "ffmpeg" for tests - assumes FFmpeg is in PATH for test environment
        DeviceDiscovery::new("ffmpeg".to_string())
    }

    #[test]
    fn test_list_cameras() {
        // This test will work on any platform
        let discovery = test_discovery();
        let result = discovery.list_cameras();
        // Should not error, but may return empty list if no cameras
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_displays() {
        let discovery = test_discovery();
        let result = discovery.list_displays();
        assert!(result.is_ok());
        // Should always have at least one display
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    fn test_cache_validity() {
        let cache = CachedDevices {
            cameras: vec![],
            displays: vec![],
            audio_devices: vec![],
            capture_cards: vec![],
            last_update: Some(Instant::now()),
        };
        assert!(cache.is_valid());

        let old_cache = CachedDevices {
            cameras: vec![],
            displays: vec![],
            audio_devices: vec![],
            capture_cards: vec![],
            last_update: Some(Instant::now() - Duration::from_secs(60)),
        };
        assert!(!old_cache.is_valid());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_parse_avfoundation_cameras() {
        let sample_output = r#"
[AVFoundation indev @ 0x7f9a8b800000] AVFoundation video devices:
[AVFoundation indev @ 0x7f9a8b800000] [0] FaceTime HD Camera
[AVFoundation indev @ 0x7f9a8b800000] [1] Capture screen 0
[AVFoundation indev @ 0x7f9a8b800000] AVFoundation audio devices:
[AVFoundation indev @ 0x7f9a8b800000] [0] Built-in Microphone
"#;
        let cameras = DeviceDiscovery::parse_avfoundation_cameras(sample_output).unwrap();
        assert_eq!(cameras.len(), 1);
        assert_eq!(cameras[0].name, "FaceTime HD Camera");
        assert_eq!(cameras[0].device_id, "0");
    }
}
