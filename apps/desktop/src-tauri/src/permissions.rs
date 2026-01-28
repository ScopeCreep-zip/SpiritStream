// Cross-Platform Permission Handling
// Provides functions to check and request camera, microphone, and screen recording permissions
// Uses native APIs: crabcamera/scap (macOS), windows crate (Windows), ashpd (Linux)

use serde::{Deserialize, Serialize};
use tauri::command;

/// Permission state across platforms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionState {
    /// Permission has been granted
    Granted,
    /// Permission has been denied by user
    Denied,
    /// Permission has not been requested yet (can prompt)
    NotDetermined,
    /// Permission is restricted by system policy (macOS parental controls, etc.)
    Restricted,
    /// Platform doesn't support checking this permission
    Unknown,
}

/// Permission types that SpiritStream may need
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionType {
    Camera,
    Microphone,
    ScreenRecording,
}

/// Permission status for all permission types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionStatus {
    pub camera: PermissionState,
    pub microphone: PermissionState,
    pub screen_recording: PermissionState,
}

impl Default for PermissionStatus {
    fn default() -> Self {
        Self {
            camera: PermissionState::Unknown,
            microphone: PermissionState::Unknown,
            screen_recording: PermissionState::Unknown,
        }
    }
}

// ============================================================================
// macOS Implementation
// ============================================================================

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use crabcamera::commands::permissions::{check_camera_permission_status, request_camera_permission};
    use crabcamera::permissions::PermissionStatus as CrabPermissionStatus;

    /// Check camera permission using the async API (safe from any thread)
    pub async fn check_camera() -> PermissionState {
        match check_camera_permission_status().await {
            Ok(info) => match info.status {
                CrabPermissionStatus::Granted => PermissionState::Granted,
                CrabPermissionStatus::Denied => PermissionState::Denied,
                CrabPermissionStatus::NotDetermined => PermissionState::NotDetermined,
                CrabPermissionStatus::Restricted => PermissionState::Restricted,
            },
            Err(e) => {
                log::warn!("Failed to check camera permission: {}", e);
                PermissionState::Unknown
            }
        }
    }

    pub fn check_screen_recording() -> PermissionState {
        // scap::has_permission() is safe to call from any thread
        if scap::has_permission() {
            PermissionState::Granted
        } else {
            // scap doesn't distinguish between denied and not determined
            PermissionState::NotDetermined
        }
    }

    pub fn check_microphone() -> PermissionState {
        // cpal (audio library) handles microphone access on first use
        // There's no pre-check API, so we return Unknown
        PermissionState::Unknown
    }

    pub async fn request_camera() -> bool {
        match request_camera_permission().await {
            Ok(info) => {
                log::info!("Camera permission request result: {:?}", info);
                matches!(info.status, CrabPermissionStatus::Granted)
            }
            Err(e) => {
                log::warn!("Camera permission request failed: {}", e);
                false
            }
        }
    }

    pub fn request_screen_recording() -> bool {
        // scap::request_permission() opens System Preferences
        // User must manually grant permission there
        scap::request_permission();
        // Return true to indicate request was initiated (not necessarily granted)
        true
    }

    pub fn request_microphone() -> bool {
        // Microphone permission is triggered by actual audio access
        // cpal will prompt when we try to open an audio input device
        true
    }
}

// ============================================================================
// Windows Implementation
// ============================================================================

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;

    pub fn check_camera() -> PermissionState {
        // Windows uses capability-based permissions
        // MediaCapture.InitializeAsync() will prompt if needed
        // We can check via AppCapability but it's complex
        // For simplicity, return Unknown and let the actual capture handle it
        PermissionState::Unknown
    }

    pub fn check_screen_recording() -> PermissionState {
        // Screen recording doesn't require permission on Windows desktop apps
        PermissionState::Granted
    }

    pub fn check_microphone() -> PermissionState {
        // Similar to camera - let WASAPI/cpal handle it
        PermissionState::Unknown
    }

    pub async fn request_camera() -> bool {
        // On Windows, MediaCapture.InitializeAsync() triggers the consent prompt
        // We'll attempt to initialize and return success/failure
        use windows::Media::Capture::MediaCapture;

        match MediaCapture::new() {
            Ok(capture) => {
                match capture.InitializeAsync() {
                    Ok(op) => {
                        // Wait for the async operation
                        match op.get() {
                            Ok(_) => true,
                            Err(e) => {
                                log::warn!("Failed to initialize camera: {:?}", e);
                                false
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to start camera initialization: {:?}", e);
                        false
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to create MediaCapture: {:?}", e);
                false
            }
        }
    }

    pub fn request_screen_recording() -> bool {
        // No permission needed on Windows
        true
    }

    pub async fn request_microphone() -> bool {
        // Same as camera - MediaCapture can handle audio
        // Or we let cpal handle it on first use
        true
    }
}

// ============================================================================
// Linux Implementation
// ============================================================================

#[cfg(target_os = "linux")]
mod linux {
    use super::*;

    pub async fn check_camera() -> PermissionState {
        // On Linux, camera access is typically group-based (video group)
        // xdg-desktop-portal handles the permission dialog when we request
        use ashpd::desktop::camera::Camera;

        match Camera::new().await {
            Ok(camera) => {
                // is_present() tells us if a camera exists, not permission state
                match camera.is_present().await {
                    Ok(present) => {
                        if present {
                            // Camera exists, permission state unknown until we request
                            PermissionState::Unknown
                        } else {
                            // No camera device
                            PermissionState::Denied
                        }
                    }
                    Err(_) => PermissionState::Unknown,
                }
            }
            Err(_) => PermissionState::Unknown,
        }
    }

    pub fn check_screen_recording() -> PermissionState {
        // Screen recording on Linux uses xdg-desktop-portal
        // Permission is granted per-session via the portal picker
        PermissionState::Unknown
    }

    pub fn check_microphone() -> PermissionState {
        // Audio access is group-based (audio group) on most Linux systems
        // PulseAudio/PipeWire handle access
        PermissionState::Unknown
    }

    pub async fn request_camera() -> bool {
        use ashpd::desktop::camera::Camera;

        match Camera::new().await {
            Ok(camera) => {
                match camera.request_access().await {
                    Ok(_) => {
                        log::info!("Camera access granted via xdg-desktop-portal");
                        true
                    }
                    Err(e) => {
                        log::warn!("Camera access denied: {:?}", e);
                        false
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to connect to camera portal: {:?}", e);
                false
            }
        }
    }

    pub fn request_screen_recording() -> bool {
        // Screen recording permission is granted when creating a screencast session
        // via xdg-desktop-portal. The actual capture code handles this.
        // Just return true to indicate the request flow should continue.
        true
    }

    pub fn request_microphone() -> bool {
        // Audio is handled by PulseAudio/PipeWire
        // Permission is typically group-based
        true
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Check all permission statuses
#[command]
pub async fn check_permissions() -> PermissionStatus {
    #[cfg(target_os = "macos")]
    {
        PermissionStatus {
            camera: macos::check_camera().await,
            microphone: macos::check_microphone(),
            screen_recording: macos::check_screen_recording(),
        }
    }

    #[cfg(target_os = "windows")]
    {
        PermissionStatus {
            camera: windows_impl::check_camera(),
            microphone: windows_impl::check_microphone(),
            screen_recording: windows_impl::check_screen_recording(),
        }
    }

    #[cfg(target_os = "linux")]
    {
        PermissionStatus {
            camera: linux::check_camera().await,
            microphone: linux::check_microphone(),
            screen_recording: linux::check_screen_recording(),
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        // Other platforms - assume permissions are granted
        PermissionStatus {
            camera: PermissionState::Granted,
            microphone: PermissionState::Granted,
            screen_recording: PermissionState::Granted,
        }
    }
}

/// Request a specific permission
/// Returns true if permission was granted or the request was initiated
#[command]
pub async fn request_permission(perm_type: PermissionType) -> Result<bool, String> {
    log::info!("Requesting {:?} permission", perm_type);

    #[cfg(target_os = "macos")]
    {
        let result = match perm_type {
            PermissionType::Camera => macos::request_camera().await,
            PermissionType::Microphone => macos::request_microphone(),
            PermissionType::ScreenRecording => macos::request_screen_recording(),
        };
        Ok(result)
    }

    #[cfg(target_os = "windows")]
    {
        let result = match perm_type {
            PermissionType::Camera => windows_impl::request_camera().await,
            PermissionType::Microphone => windows_impl::request_microphone().await,
            PermissionType::ScreenRecording => windows_impl::request_screen_recording(),
        };
        Ok(result)
    }

    #[cfg(target_os = "linux")]
    {
        let result = match perm_type {
            PermissionType::Camera => linux::request_camera().await,
            PermissionType::Microphone => linux::request_microphone(),
            PermissionType::ScreenRecording => linux::request_screen_recording(),
        };
        Ok(result)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = perm_type;
        Ok(true)
    }
}

/// Get platform-specific guidance for enabling a permission
#[command]
pub fn get_permission_guidance(perm_type: PermissionType) -> String {
    #[cfg(target_os = "macos")]
    {
        match perm_type {
            PermissionType::Camera => {
                "Open System Settings > Privacy & Security > Camera and enable SpiritStream".into()
            }
            PermissionType::Microphone => {
                "Open System Settings > Privacy & Security > Microphone and enable SpiritStream".into()
            }
            PermissionType::ScreenRecording => {
                "Open System Settings > Privacy & Security > Screen Recording and enable SpiritStream".into()
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        match perm_type {
            PermissionType::Camera => {
                "Open Settings > Privacy > Camera and ensure camera access is enabled".into()
            }
            PermissionType::Microphone => {
                "Open Settings > Privacy > Microphone and ensure microphone access is enabled".into()
            }
            PermissionType::ScreenRecording => {
                "Screen recording does not require special permissions on Windows".into()
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        match perm_type {
            PermissionType::Camera => {
                "Ensure your user is in the 'video' group: sudo usermod -aG video $USER (then log out and back in)".into()
            }
            PermissionType::Microphone => {
                "Ensure your user is in the 'audio' group: sudo usermod -aG audio $USER (then log out and back in)".into()
            }
            PermissionType::ScreenRecording => {
                "Ensure xdg-desktop-portal is installed for your desktop environment (gnome, kde, wlr)".into()
            }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = perm_type;
        "Permission management is not available on this platform".into()
    }
}

/// Check if we're on macOS (useful for frontend to show platform-specific UI)
#[command]
pub fn get_platform() -> String {
    #[cfg(target_os = "macos")]
    { "macos".into() }

    #[cfg(target_os = "windows")]
    { "windows".into() }

    #[cfg(target_os = "linux")]
    { "linux".into() }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    { "unknown".into() }
}
