// Permissions Service
// Handles platform-specific permission checks and requests

use serde::{Deserialize, Serialize};

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
    /// Permission has not been requested yet
    NotDetermined,
    /// Permission is restricted by system policy
    Restricted,
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
}

/// Service for checking and requesting permissions
pub struct PermissionsService;

impl PermissionsService {
    /// Get the current status of all permissions
    pub fn get_status() -> PermissionStatusReport {
        PermissionStatusReport {
            camera: Self::check_camera_permission(),
            microphone: Self::check_microphone_permission(),
            screen_recording: Self::check_screen_recording_permission(),
            platform: Self::platform_name().to_string(),
        }
    }

    /// Get platform name
    fn platform_name() -> &'static str {
        #[cfg(target_os = "macos")]
        { "macos" }
        #[cfg(target_os = "windows")]
        { "windows" }
        #[cfg(target_os = "linux")]
        { "linux" }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        { "unknown" }
    }

    /// Check camera permission status
    pub fn check_camera_permission() -> PermissionStatus {
        #[cfg(target_os = "macos")]
        {
            // On macOS, camera permission is checked via AVFoundation
            // For now, return NotDetermined - actual check requires linking to AVFoundation
            // The permission will be requested when camera capture is first attempted
            PermissionStatus::NotDetermined
        }

        #[cfg(target_os = "windows")]
        {
            // Windows doesn't require explicit camera permission for desktop apps
            PermissionStatus::Granted
        }

        #[cfg(target_os = "linux")]
        {
            // Linux V4L2 doesn't require explicit permission
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
            // Linux audio doesn't require explicit permission
            PermissionStatus::Granted
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            PermissionStatus::NotApplicable
        }
    }

    /// Check screen recording permission status
    pub fn check_screen_recording_permission() -> PermissionStatus {
        #[cfg(target_os = "macos")]
        {
            // Use scap's permission check for screen recording
            if scap::has_permission() {
                PermissionStatus::Granted
            } else {
                PermissionStatus::NotDetermined
            }
        }

        #[cfg(target_os = "windows")]
        {
            // Windows doesn't require explicit screen capture permission
            PermissionStatus::Granted
        }

        #[cfg(target_os = "linux")]
        {
            // X11 doesn't require permission, Wayland will show portal
            PermissionStatus::Granted
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            PermissionStatus::NotApplicable
        }
    }

    /// Request camera permission
    /// Returns true if permission was granted or already granted
    pub fn request_camera_permission() -> bool {
        #[cfg(target_os = "macos")]
        {
            // On macOS, the permission dialog is shown when capture starts
            // We can't programmatically request it without AVFoundation
            log::info!("Camera permission will be requested on first use");
            true
        }

        #[cfg(not(target_os = "macos"))]
        {
            true
        }
    }

    /// Request microphone permission
    /// Returns true if permission was granted or already granted
    pub fn request_microphone_permission() -> bool {
        #[cfg(target_os = "macos")]
        {
            // On macOS, the permission dialog is shown when capture starts
            log::info!("Microphone permission will be requested on first use");
            true
        }

        #[cfg(not(target_os = "macos"))]
        {
            true
        }
    }

    /// Request screen recording permission
    /// On macOS, this opens System Preferences to the Screen Recording pane
    /// Returns true if permission was granted (may need app restart on macOS)
    pub fn request_screen_recording_permission() -> bool {
        #[cfg(target_os = "macos")]
        {
            // Use scap's request_permission which opens System Preferences
            let granted = scap::request_permission();
            if !granted {
                log::info!("Screen recording permission requested - user must grant in System Preferences");
            }
            granted
        }

        #[cfg(not(target_os = "macos"))]
        {
            true
        }
    }

    /// Request all permissions at once
    /// Returns a report of which permissions were granted
    pub fn request_all_permissions() -> PermissionStatusReport {
        Self::request_camera_permission();
        Self::request_microphone_permission();
        Self::request_screen_recording_permission();

        Self::get_status()
    }

    /// Check if screen capture is supported on this platform
    pub fn is_screen_capture_supported() -> bool {
        scap::is_supported()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_status() {
        let status = PermissionsService::get_status();
        // Should return some status without panicking
        assert!(!status.platform.is_empty());
    }

    #[test]
    fn test_is_screen_capture_supported() {
        // Should not panic
        let _ = PermissionsService::is_screen_capture_supported();
    }
}
