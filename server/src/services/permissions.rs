// Permissions Service
// Handles platform-specific permission checks and requests
//
// Threading Requirements by Platform:
// - macOS: has_permission() can block, request_permission() needs main thread for UI
// - Windows: No pre-check needed, system shows picker when capture starts
// - Linux: No pre-check needed, XDG portal shows picker when capture starts
//
// Architecture:
// - Permission CHECKS: Run via spawn_blocking on macOS (can block)
// - Permission REQUESTS: Return guidance (actual UI must be triggered by desktop layer)
// - Device LISTING: Safe on all threads (scap::get_all_targets is thread-safe)
// - Actual CAPTURE: Safe on worker threads (uses dispatch queues internally)

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Permission types that may require user approval
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionType {
    Camera,
    Microphone,
    ScreenRecording,
}

/// Permission status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionStatus {
    /// Permission has been granted
    Granted,
    /// Permission has been denied
    Denied,
    /// Permission has not been requested yet (macOS)
    NotDetermined,
    /// Permission is restricted by system policy
    Restricted,
    /// Permission uses picker-based flow (Windows/Linux) - no pre-check needed
    PickerBased,
    /// Permission checking is not supported on this platform
    NotApplicable,
}

/// Overall permission status for all capture types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionStatusReport {
    pub camera: PermissionStatus,
    pub microphone: PermissionStatus,
    pub screen_recording: PermissionStatus,
    pub platform: String,
    /// Human-readable guidance for requesting permissions
    pub guidance: Option<String>,
}

/// Cached permission status for macOS (checked once at startup on main thread)
static CACHED_SCREEN_PERMISSION: OnceLock<bool> = OnceLock::new();

/// Service for checking and requesting permissions
pub struct PermissionsService;

impl PermissionsService {
    /// Get platform name
    pub fn platform_name() -> &'static str {
        #[cfg(target_os = "macos")]
        {
            "macos"
        }
        #[cfg(target_os = "windows")]
        {
            "windows"
        }
        #[cfg(target_os = "linux")]
        {
            "linux"
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            "unknown"
        }
    }

    /// Get the current status of all permissions (sync version - uses cached values on macOS)
    /// For accurate macOS status, use get_status_async() instead
    pub fn get_status() -> PermissionStatusReport {
        PermissionStatusReport {
            camera: Self::check_camera_permission(),
            microphone: Self::check_microphone_permission(),
            screen_recording: Self::check_screen_recording_permission_cached(),
            platform: Self::platform_name().to_string(),
            guidance: Self::get_platform_guidance(),
        }
    }

    /// Get the current status of all permissions (async version - safe for tokio)
    /// On macOS, this runs the scap check in a blocking task
    pub async fn get_status_async() -> PermissionStatusReport {
        let screen_recording = Self::check_screen_recording_permission_async().await;

        PermissionStatusReport {
            camera: Self::check_camera_permission(),
            microphone: Self::check_microphone_permission(),
            screen_recording,
            platform: Self::platform_name().to_string(),
            guidance: Self::get_platform_guidance(),
        }
    }

    /// Get platform-specific guidance for the user
    fn get_platform_guidance() -> Option<String> {
        #[cfg(target_os = "macos")]
        {
            Some(
                "On macOS, you need to grant Screen Recording permission in System Settings > \
                Privacy & Security > Screen Recording. The app may need to be restarted after \
                granting permission."
                    .to_string(),
            )
        }
        #[cfg(target_os = "windows")]
        {
            Some(
                "On Windows, you'll be prompted to select which screen or window to capture \
                when you start a capture. No additional permissions are required."
                    .to_string(),
            )
        }
        #[cfg(target_os = "linux")]
        {
            Some(
                "On Linux, your desktop environment will show a portal dialog to select \
                which screen or window to share when you start a capture."
                    .to_string(),
            )
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            None
        }
    }

    /// Check camera permission status
    pub fn check_camera_permission() -> PermissionStatus {
        #[cfg(target_os = "macos")]
        {
            // On macOS, camera permission is checked via AVFoundation
            // The permission dialog is shown when camera capture is first attempted
            // We return NotDetermined since we can't check without AVFoundation bindings
            PermissionStatus::NotDetermined
        }

        #[cfg(target_os = "windows")]
        {
            // Windows uses a picker - no pre-check needed
            PermissionStatus::PickerBased
        }

        #[cfg(target_os = "linux")]
        {
            // Linux V4L2 doesn't require explicit permission for cameras
            PermissionStatus::Granted
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            PermissionStatus::NotApplicable
        }
    }

    /// Check microphone permission status
    pub fn check_microphone_permission() -> PermissionStatus {
        #[cfg(target_os = "macos")]
        {
            // On macOS, microphone permission is checked via AVFoundation
            // Return NotDetermined - will be requested on first use
            PermissionStatus::NotDetermined
        }

        #[cfg(target_os = "windows")]
        {
            // Windows doesn't require explicit microphone permission for desktop apps
            PermissionStatus::Granted
        }

        #[cfg(target_os = "linux")]
        {
            // Linux audio (PipeWire/PulseAudio) doesn't require explicit permission
            PermissionStatus::Granted
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            PermissionStatus::NotApplicable
        }
    }

    /// Check screen recording permission (cached version - safe for sync contexts)
    /// Returns cached value on macOS, or checks directly on other platforms
    pub fn check_screen_recording_permission_cached() -> PermissionStatus {
        #[cfg(target_os = "macos")]
        {
            // Use cached value if available, otherwise return NotDetermined
            // The cache is populated by check_screen_recording_permission_async() or init_permission_cache()
            match CACHED_SCREEN_PERMISSION.get() {
                Some(true) => PermissionStatus::Granted,
                Some(false) => PermissionStatus::NotDetermined,
                None => PermissionStatus::NotDetermined,
            }
        }

        #[cfg(target_os = "windows")]
        {
            // Windows uses a picker dialog - no pre-check needed
            PermissionStatus::PickerBased
        }

        #[cfg(target_os = "linux")]
        {
            // Linux uses XDG portal picker - no pre-check needed
            PermissionStatus::PickerBased
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            PermissionStatus::NotApplicable
        }
    }

    /// Check screen recording permission (async version - safe for tokio worker threads)
    /// On macOS, this runs the scap check in spawn_blocking with timeout protection
    /// since SCShareableContent can hang for 3-10 seconds or never return
    pub async fn check_screen_recording_permission_async() -> PermissionStatus {
        #[cfg(target_os = "macos")]
        {
            use std::time::Duration;

            // Run scap::has_permission() in a blocking task with timeout
            // SCShareableContent can hang for 3-10+ seconds on macOS
            let check_future = tokio::task::spawn_blocking(|| {
                // Use catch_unwind to handle any panics from scap
                std::panic::catch_unwind(|| scap::has_permission()).unwrap_or(false)
            });

            // Apply 5 second timeout - if it takes longer, assume permission denied
            let result = match tokio::time::timeout(Duration::from_secs(5), check_future).await {
                Ok(Ok(has_perm)) => has_perm,
                Ok(Err(_join_err)) => {
                    log::warn!("Permission check task panicked");
                    false
                }
                Err(_timeout) => {
                    log::warn!("Permission check timed out after 5 seconds");
                    false
                }
            };

            // Cache the result for future sync calls
            let _ = CACHED_SCREEN_PERMISSION.set(result);

            if result {
                PermissionStatus::Granted
            } else {
                PermissionStatus::NotDetermined
            }
        }

        #[cfg(target_os = "windows")]
        {
            PermissionStatus::PickerBased
        }

        #[cfg(target_os = "linux")]
        {
            PermissionStatus::PickerBased
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            PermissionStatus::NotApplicable
        }
    }

    /// Initialize the permission cache.
    ///
    /// DEPRECATED: Do NOT call this from within the tokio runtime (e.g., in main()).
    /// The scap::has_permission() call can block for 3-10 seconds and will starve
    /// other tasks if called from async context. Use check_screen_recording_permission_async()
    /// instead, which handles this via spawn_blocking with timeout protection.
    #[cfg(target_os = "macos")]
    #[deprecated(note = "Use check_screen_recording_permission_async() instead - safe for async context")]
    pub fn init_permission_cache() {
        if CACHED_SCREEN_PERMISSION.get().is_none() {
            log::warn!("init_permission_cache called - this can block. Prefer async methods.");
            let result = std::panic::catch_unwind(|| scap::has_permission()).unwrap_or(false);
            let _ = CACHED_SCREEN_PERMISSION.set(result);
            log::info!(
                "Screen recording permission cache initialized: {}",
                if result { "granted" } else { "not granted" }
            );
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[deprecated(note = "Use check_screen_recording_permission_async() instead")]
    pub fn init_permission_cache() {
        // No-op on non-macOS platforms
    }

    /// Request camera permission
    /// Returns true if permission is available (granted or will be prompted on use)
    pub fn request_camera_permission() -> bool {
        #[cfg(target_os = "macos")]
        {
            // On macOS, the permission dialog is shown when capture starts
            // We can't programmatically trigger it without AVFoundation
            log::info!("Camera permission will be requested when capture starts");
            true
        }

        #[cfg(not(target_os = "macos"))]
        {
            true
        }
    }

    /// Request microphone permission
    /// Returns true if permission is available (granted or will be prompted on use)
    pub fn request_microphone_permission() -> bool {
        #[cfg(target_os = "macos")]
        {
            // On macOS, the permission dialog is shown when capture starts
            log::info!("Microphone permission will be requested when capture starts");
            true
        }

        #[cfg(not(target_os = "macos"))]
        {
            true
        }
    }

    /// Request screen recording permission (async version)
    /// On macOS, this runs scap::request_permission() in spawn_blocking with timeout
    /// Note: On macOS, this opens System Preferences - user must grant permission manually
    /// The function returns immediately after opening preferences (doesn't wait for user)
    pub async fn request_screen_recording_permission_async() -> bool {
        #[cfg(target_os = "macos")]
        {
            use std::time::Duration;

            let request_future = tokio::task::spawn_blocking(|| {
                // Use catch_unwind to handle any panics from scap
                std::panic::catch_unwind(|| scap::request_permission()).unwrap_or(false)
            });

            // Apply 5 second timeout
            let result = match tokio::time::timeout(Duration::from_secs(5), request_future).await {
                Ok(Ok(granted)) => granted,
                Ok(Err(_join_err)) => {
                    log::warn!("Permission request task panicked");
                    false
                }
                Err(_timeout) => {
                    log::warn!("Permission request timed out after 5 seconds");
                    false
                }
            };

            if !result {
                log::info!(
                    "Screen recording permission requested - user must grant in System Settings"
                );
            }

            // Update cache
            let _ = CACHED_SCREEN_PERMISSION.set(result);

            result
        }

        #[cfg(not(target_os = "macos"))]
        {
            // Windows/Linux use picker-based flow, always return true
            true
        }
    }

    /// Check if screen capture is supported on this platform
    pub fn is_screen_capture_supported() -> bool {
        #[cfg(target_os = "macos")]
        {
            // Run in catch_unwind in case scap panics
            std::panic::catch_unwind(|| scap::is_supported()).unwrap_or(false)
        }

        #[cfg(not(target_os = "macos"))]
        {
            scap::is_supported()
        }
    }

    /// Check if we need to show a permission prompt before capture
    /// Returns true only on macOS where explicit permission is required
    pub fn needs_permission_prompt() -> bool {
        #[cfg(target_os = "macos")]
        {
            true
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_name() {
        let name = PermissionsService::platform_name();
        assert!(!name.is_empty());
        #[cfg(target_os = "macos")]
        assert_eq!(name, "macos");
        #[cfg(target_os = "windows")]
        assert_eq!(name, "windows");
        #[cfg(target_os = "linux")]
        assert_eq!(name, "linux");
    }

    #[test]
    fn test_get_status_sync() {
        // Sync version should not panic
        let status = PermissionsService::get_status();
        assert!(!status.platform.is_empty());
    }

    #[tokio::test]
    async fn test_get_status_async() {
        // Async version should not panic
        let status = PermissionsService::get_status_async().await;
        assert!(!status.platform.is_empty());
    }

    #[test]
    fn test_needs_permission_prompt() {
        let needs_prompt = PermissionsService::needs_permission_prompt();
        #[cfg(target_os = "macos")]
        assert!(needs_prompt);
        #[cfg(not(target_os = "macos"))]
        assert!(!needs_prompt);
    }
}
