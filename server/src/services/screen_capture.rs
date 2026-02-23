// Screen Capture Service
// Uses scap for native screen capture with OS permission handling
//
// Threading Safety:
// - list_displays(), list_windows(): Safe on any thread (scap::get_all_targets is thread-safe)
// - has_permission(): Use has_permission_cached() for sync contexts, or check via PermissionsService
// - start_*_capture(): Safe - capture runs in dedicated thread with dispatch queues
// - stop_capture(): Safe on any thread
//
// Performance Notes:
// - scap::get_all_targets() can block for 3-10 seconds on macOS (ScreenCaptureKit enumeration)
// - We cache targets with a 5-second TTL to avoid repeated blocking calls

use scap::{
    capturer::{Capturer, Options, Resolution},
    frame::{Frame, FrameType},
    Target,
};
use std::sync::{Arc, OnceLock, RwLock};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

use super::capture_core::session::{CaptureSession, CaptureSessionManager};
use super::capture_core::timeout::spawn_blocking_with_timeout_or_default;

use super::permissions::PermissionsService;

/// Cache TTL for scap targets (5 seconds)
const TARGET_CACHE_TTL: Duration = Duration::from_secs(5);

/// Cached scap targets with timestamp
/// Uses Arc<Vec<Target>> so cache reads are a cheap pointer clone instead of deep copy
struct TargetCache {
    targets: Arc<Vec<Target>>,
    last_refresh: Instant,
}

/// Global cache for scap targets to avoid repeated 3-10s blocking calls
static TARGET_CACHE: OnceLock<RwLock<Option<TargetCache>>> = OnceLock::new();

fn get_target_cache() -> &'static RwLock<Option<TargetCache>> {
    TARGET_CACHE.get_or_init(|| RwLock::new(None))
}

/// Get scap targets with caching
/// Returns Arc-wrapped targets — pointer copy instead of deep clone on cache hit
fn get_targets_cached() -> Arc<Vec<Target>> {
    let cache = get_target_cache();

    // Try to read from cache (cheap Arc::clone on hit)
    {
        if let Ok(guard) = cache.read() {
            if let Some(ref c) = *guard {
                if c.last_refresh.elapsed() < TARGET_CACHE_TTL {
                    return Arc::clone(&c.targets);
                }
            }
        }
    }

    // Cache miss or expired - refresh
    let targets = Arc::new(scap::get_all_targets());

    // Update cache
    if let Ok(mut guard) = cache.write() {
        *guard = Some(TargetCache {
            targets: Arc::clone(&targets),
            last_refresh: Instant::now(),
        });
    }

    targets
}

/// Build a map of display_id → (width, height) using screencapturekit's SCDisplay
/// which provides accurate pixel dimensions. Falls back to empty map on error.
#[cfg(target_os = "macos")]
fn get_display_dimensions_map() -> std::collections::HashMap<u32, (u32, u32)> {
    use screencapturekit::shareable_content::SCShareableContent;

    match SCShareableContent::get() {
        Ok(content) => content
            .displays()
            .iter()
            .map(|d| (d.display_id(), (d.width(), d.height())))
            .collect(),
        Err(e) => {
            log::debug!("Failed to get SCShareableContent for dimensions: {:?}", e);
            std::collections::HashMap::new()
        }
    }
}

/// Invalidate the target cache (e.g., when displays are added/removed)
#[allow(dead_code)]
pub fn invalidate_target_cache() {
    if let Ok(mut guard) = get_target_cache().write() {
        *guard = None;
    }
}

/// Information about an available display
/// Carries both the native scap ID (for capture) and profile-facing fields (for frontend/storage)
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayInfo {
    /// Native scap ID (CGDirectDisplayID on macOS, HMONITOR on Windows)
    pub id: u32,
    /// Native ID as string for profile storage / JSON compat
    pub display_id: String,
    /// Human-readable name (e.g., "Built-in Retina Display")
    pub name: String,
    /// Platform device name for go2rtc (e.g., "Capture screen 0" on macOS)
    /// Populated by DeviceDiscovery's AVFoundation enrichment, not by scap
    pub device_name: Option<String>,
    /// Display width in pixels
    pub width: u32,
    /// Display height in pixels
    pub height: u32,
    /// Whether this is the primary display
    pub is_primary: bool,
}

/// Information about an available window
#[derive(Debug, Clone, serde::Serialize)]
pub struct WindowInfo {
    pub id: u32,
    pub title: String,
}

/// Screen capture configuration
#[derive(Debug, Clone)]
pub struct ScreenCaptureConfig {
    pub fps: u32,
    pub show_cursor: bool,
    pub show_highlight: bool,
    pub output_resolution: Resolution,
}

impl Default for ScreenCaptureConfig {
    fn default() -> Self {
        Self {
            fps: 30,
            show_cursor: true,
            show_highlight: false,
            output_resolution: Resolution::_1080p,
        }
    }
}

/// Service for managing screen capture
pub struct ScreenCaptureService {
    sessions: CaptureSessionManager<Frame>,
}

impl ScreenCaptureService {
    pub fn new() -> Self {
        Self {
            sessions: CaptureSessionManager::new("screen"),
        }
    }

    /// Check if screen capture is supported on this platform
    pub fn is_supported() -> bool {
        PermissionsService::is_screen_capture_supported()
    }

    /// Check if screen capture permission has been granted (cached, safe for sync contexts)
    /// For async contexts, use PermissionsService::check_screen_recording_permission_async()
    pub fn has_permission_cached() -> bool {
        use super::permissions::PermissionStatus;
        matches!(
            PermissionsService::check_screen_recording_permission_cached(),
            PermissionStatus::Granted | PermissionStatus::PickerBased
        )
    }

    /// Check if screen capture permission has been granted (async, safe for tokio)
    pub async fn has_permission_async() -> bool {
        use super::permissions::PermissionStatus;
        matches!(
            PermissionsService::check_screen_recording_permission_async().await,
            PermissionStatus::Granted | PermissionStatus::PickerBased
        )
    }

    /// Request screen capture permission (async, safe for tokio)
    /// On macOS, this opens System Preferences - user must grant permission manually
    pub async fn request_permission_async() -> bool {
        PermissionsService::request_screen_recording_permission_async().await
    }

    /// List available displays/monitors (sync version - use list_displays_async in async contexts)
    /// Uses cached targets to avoid repeated 3-10s blocking calls on macOS
    /// Returns native scap IDs (CGDirectDisplayID on macOS, HMONITOR on Windows)
    pub fn list_displays() -> Vec<DisplayInfo> {
        let targets = get_targets_cached();

        // On macOS, get display dimensions from screencapturekit (SCDisplay has width/height)
        #[cfg(target_os = "macos")]
        let dimension_map = get_display_dimensions_map();

        let mut is_first = true;

        targets
            .iter()
            .filter_map(|target| {
                if let Target::Display(display) = target {
                    let is_primary = is_first;
                    is_first = false;

                    // Look up dimensions from screencapturekit on macOS, default on other platforms
                    #[cfg(target_os = "macos")]
                    let (width, height) = dimension_map
                        .get(&display.id)
                        .copied()
                        .unwrap_or((1920, 1080));

                    #[cfg(not(target_os = "macos"))]
                    let (width, height) = (1920u32, 1080u32);

                    Some(DisplayInfo {
                        id: display.id,
                        display_id: display.id.to_string(),
                        name: display.title.clone(),
                        device_name: None, // Enriched by DeviceDiscovery with AVFoundation names
                        width,
                        height,
                        is_primary,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// List available displays/monitors (async version - safe for tokio)
    /// Uses spawn_blocking with timeout protection since scap can hang
    pub async fn list_displays_async() -> Vec<DisplayInfo> {
        spawn_blocking_with_timeout_or_default("list_displays", 5, Self::list_displays).await
    }

    /// List available windows (sync version - use list_windows_async in async contexts)
    /// Uses cached targets to avoid repeated 3-10s blocking calls on macOS
    pub fn list_windows() -> Vec<WindowInfo> {
        let targets = get_targets_cached();

        targets
            .iter()
            .filter_map(|target| {
                if let Target::Window(window) = target {
                    Some(WindowInfo {
                        id: window.id,
                        title: window.title.clone(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// List available windows (async version - safe for tokio)
    /// Uses spawn_blocking with timeout protection since scap can hang
    pub async fn list_windows_async() -> Vec<WindowInfo> {
        spawn_blocking_with_timeout_or_default("list_windows", 5, Self::list_windows).await
    }

    /// Start capturing a display
    pub fn start_display_capture(
        &self,
        display_id: u32,
        config: ScreenCaptureConfig,
    ) -> Result<broadcast::Receiver<Arc<Frame>>, String> {
        let capture_id = format!("display_{}", display_id);

        // Find the display (uses cached targets for performance)
        let targets = get_targets_cached();
        let target = targets
            .iter()
            .find_map(|t| {
                if let Target::Display(d) = t {
                    if d.id == display_id { Some(Target::Display(d.clone())) } else { None }
                } else { None }
            })
            .ok_or_else(|| format!("Display {} not found", display_id))?;

        self.start_capture_internal(capture_id, target, config)
    }

    /// Start capturing a window
    pub fn start_window_capture(
        &self,
        window_id: u32,
        config: ScreenCaptureConfig,
    ) -> Result<broadcast::Receiver<Arc<Frame>>, String> {
        let capture_id = format!("window_{}", window_id);

        // Find the window (uses cached targets for performance)
        let targets = get_targets_cached();
        let target = targets
            .iter()
            .find_map(|t| {
                if let Target::Window(w) = t {
                    if w.id == window_id { Some(Target::Window(w.clone())) } else { None }
                } else { None }
            })
            .ok_or_else(|| format!("Window {} not found", window_id))?;

        self.start_capture_internal(capture_id, target, config)
    }

    /// Internal: start capture for any target type (display or window)
    fn start_capture_internal(
        &self,
        capture_id: String,
        target: Target,
        config: ScreenCaptureConfig,
    ) -> Result<broadcast::Receiver<Arc<Frame>>, String> {
        // Check permission first (uses cached value on macOS, picker-based on Windows/Linux)
        if !Self::has_permission_cached() {
            return Err("Screen capture permission not granted. Please grant permission in System Settings > Privacy & Security > Screen Recording.".to_string());
        }

        // If already capturing this target, return a new subscriber to the existing stream
        if let Some(rx) = self.sessions.subscribe(&capture_id) {
            log::debug!("Reusing existing capture for {} (new subscriber)", capture_id);
            return Ok(rx);
        }

        // Create capturer options
        let options = Options {
            fps: config.fps,
            show_cursor: config.show_cursor,
            show_highlight: config.show_highlight,
            target: Some(target),
            excluded_targets: None,
            output_type: FrameType::BGRAFrame,
            output_resolution: config.output_resolution,
            crop_area: None,
            ..Default::default()
        };

        // Create capturer
        let mut capturer = Capturer::build(options)
            .map_err(|e| format!("Failed to build capturer: {:?}", e))?;

        // Create session with broadcast channel
        let (session, frame_rx) = CaptureSession::<Frame>::new(
            capture_id.clone(),
            "screen",
            16,
        );

        let stop_flag = session.stop_flag();
        let frame_tx = session.sender();

        // Start capture in background thread
        let thread_name = format!("ss-screen-{}", &capture_id[..capture_id.len().min(8)]);
        let handle = std::thread::Builder::new().name(thread_name).spawn(move || {
            crate::services::thread_config::set_thread_qos(crate::services::thread_config::QosClass::UserInitiated);
            capturer.start_capture();
            let mut capturing = true;

            while !stop_flag.load(Ordering::Relaxed) {
                // Pause/resume scap capturer based on consumer count to avoid CPU waste
                if frame_tx.receiver_count() == 0 {
                    if capturing {
                        capturer.stop_capture();
                        capturing = false;
                        log::debug!("Screen capture paused - no consumers");
                    }
                    std::thread::sleep(Duration::from_millis(500));
                    continue;
                }
                if !capturing {
                    capturer.start_capture();
                    capturing = true;
                    log::debug!("Screen capture resumed - consumer connected");
                }
                if let Ok(frame) = capturer.get_next_frame() {
                    let _ = frame_tx.send(Arc::new(frame));
                }
            }

            if capturing {
                capturer.stop_capture();
            }
        }).expect("Failed to spawn screen capture thread");

        // Store session with thread handle
        self.sessions.insert(capture_id, session.with_handle(handle));

        Ok(frame_rx)
    }

    /// Stop a capture by ID
    pub fn stop_capture(&self, capture_id: &str) -> Result<(), String> {
        self.sessions.stop_nonblocking(capture_id)
    }

    /// Stop all active captures
    pub fn stop_all(&self) {
        self.sessions.stop_all_nonblocking();
    }

    /// Check if a capture is active
    pub fn is_capturing(&self, capture_id: &str) -> bool {
        self.sessions.is_active(capture_id)
    }

    /// Get count of active captures
    pub fn active_capture_count(&self) -> usize {
        self.sessions.active_count()
    }

    /// Get list of active capture IDs
    pub fn active_capture_ids(&self) -> Vec<String> {
        self.sessions.active_ids()
    }

    /// Subscribe to an existing capture's broadcast channel
    pub fn subscribe(&self, capture_id: &str) -> Option<broadcast::Receiver<Arc<Frame>>> {
        self.sessions.subscribe(capture_id)
    }
}

impl Default for ScreenCaptureService {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ScreenCaptureService {
    fn drop(&mut self) {
        self.stop_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_supported() {
        let supported = ScreenCaptureService::is_supported();
        println!("Screen capture supported: {}", supported);
    }

    #[test]
    fn test_list_displays() {
        if ScreenCaptureService::is_supported() && ScreenCaptureService::has_permission_cached() {
            let displays = ScreenCaptureService::list_displays();
            println!("Found {} displays:", displays.len());
            for display in &displays {
                println!("  - {} (ID: {}, {}x{}, primary: {})", display.name, display.id, display.width, display.height, display.is_primary);
            }
        }
    }

    #[test]
    fn test_list_windows() {
        if ScreenCaptureService::is_supported() && ScreenCaptureService::has_permission_cached() {
            let windows = ScreenCaptureService::list_windows();
            println!("Found {} windows:", windows.len());
            for window in &windows[..windows.len().min(10)] {
                println!("  - {} (ID: {})", window.title, window.id);
            }
        }
    }
}
