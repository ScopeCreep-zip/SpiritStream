// Device Discovery Service
// Platform-specific device enumeration for cameras, displays, audio devices, and capture cards
// With async support and caching to prevent blocking the tokio runtime

mod cameras;
mod displays;
mod audio;
mod windows;

use crate::models::{CameraDevice, DisplayInfo, AudioInputDevice, CaptureCardDevice};
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

    /// Invalidate the cache (force next request to re-enumerate)
    pub async fn invalidate(&self) {
        let mut cache = self.devices.write().await;
        cache.last_update = None;
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
                    cameras::list_cameras_sync(&ffmpeg_path)
                })
            ).await
        });

        let displays_handle = tokio::spawn(async move {
            timeout(
                Duration::from_secs(ENUMERATION_TIMEOUT_SECS),
                tokio::task::spawn_blocking(move || {
                    displays::list_displays_sync(&ffmpeg_path2)
                })
            ).await
        });

        let audio_handle = tokio::spawn(async move {
            timeout(
                Duration::from_secs(ENUMERATION_TIMEOUT_SECS),
                tokio::task::spawn_blocking(move || {
                    audio::list_audio_inputs_sync(&ffmpeg_path3)
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
                    tokio::task::spawn_blocking(move || cameras::list_cameras_sync(&path)),
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
                    tokio::task::spawn_blocking(move || displays::list_displays_sync(&path)),
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
                    tokio::task::spawn_blocking(move || audio::list_audio_inputs_sync(&path)),
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
        cameras::list_cameras_sync(&self.ffmpeg_path)
    }

    /// List available displays (blocking)
    pub fn list_displays(&self) -> Result<Vec<DisplayInfo>, String> {
        displays::list_displays_sync(&self.ffmpeg_path)
    }

    /// List available audio input devices (blocking)
    pub fn list_audio_inputs(&self) -> Result<Vec<AudioInputDevice>, String> {
        audio::list_audio_inputs_sync(&self.ffmpeg_path)
    }

    /// List available capture cards (blocking)
    pub fn list_capture_cards(&self) -> Result<Vec<CaptureCardDevice>, String> {
        Self::list_capture_cards_sync(&self.ffmpeg_path)
    }

    // ============================================================
    // Capture card sync implementation (delegates to camera + filter)
    // ============================================================

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

    #[cfg(target_os = "macos")]
    fn list_capture_cards_macos(ffmpeg_path: &str) -> Result<Vec<CaptureCardDevice>, String> {
        // On macOS, capture cards appear as video devices in AVFoundation
        let cameras = cameras::list_cameras_sync(ffmpeg_path)?;
        Ok(self::windows::filter_capture_cards(cameras))
    }

    #[cfg(target_os = "windows")]
    fn list_capture_cards_windows(ffmpeg_path: &str) -> Result<Vec<CaptureCardDevice>, String> {
        let cameras = cameras::list_cameras_sync(ffmpeg_path)?;
        Ok(self::windows::filter_capture_cards(cameras))
    }

    #[cfg(target_os = "linux")]
    fn list_capture_cards_linux(ffmpeg_path: &str) -> Result<Vec<CaptureCardDevice>, String> {
        let cameras = cameras::list_cameras_sync(ffmpeg_path)?;
        Ok(self::windows::filter_capture_cards(cameras))
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
        let cameras = cameras::parse_avfoundation_cameras(sample_output).unwrap();
        assert_eq!(cameras.len(), 1);
        assert_eq!(cameras[0].name, "FaceTime HD Camera");
        assert_eq!(cameras[0].device_id, "0");
    }
}
