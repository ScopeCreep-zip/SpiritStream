// Source Model
// Represents different input source types for multi-input streaming

use serde::{Deserialize, Serialize};

/// Source - represents a single input source
/// Tagged enum with type discriminator for frontend compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Source {
    /// RTMP input source (incoming stream)
    Rtmp(RtmpSource),
    /// Local media file (video/audio)
    MediaFile(MediaFileSource),
    /// Screen/display capture
    ScreenCapture(ScreenCaptureSource),
    /// Camera/webcam device
    Camera(CameraSource),
    /// Capture card (HDMI capture devices like Elgato)
    CaptureCard(CaptureCardSource),
    /// Audio-only input device (microphone, line-in)
    AudioDevice(AudioDeviceSource),
}

impl Source {
    /// Get the unique ID of this source
    pub fn id(&self) -> &str {
        match self {
            Source::Rtmp(s) => &s.id,
            Source::MediaFile(s) => &s.id,
            Source::ScreenCapture(s) => &s.id,
            Source::Camera(s) => &s.id,
            Source::CaptureCard(s) => &s.id,
            Source::AudioDevice(s) => &s.id,
        }
    }

    /// Get the user-friendly name of this source
    pub fn name(&self) -> &str {
        match self {
            Source::Rtmp(s) => &s.name,
            Source::MediaFile(s) => &s.name,
            Source::ScreenCapture(s) => &s.name,
            Source::Camera(s) => &s.name,
            Source::CaptureCard(s) => &s.name,
            Source::AudioDevice(s) => &s.name,
        }
    }

    /// Check if this source has video output
    pub fn has_video(&self) -> bool {
        match self {
            Source::Rtmp(_) => true,
            Source::MediaFile(s) => !s.audio_only,
            Source::ScreenCapture(_) => true,
            Source::Camera(_) => true,
            Source::CaptureCard(_) => true,
            Source::AudioDevice(_) => false,
        }
    }

    /// Check if this source has audio output
    pub fn has_audio(&self) -> bool {
        match self {
            Source::Rtmp(_) => true,
            Source::MediaFile(_) => true,
            Source::ScreenCapture(s) => s.capture_audio,
            Source::Camera(_) => false, // Cameras typically don't have audio
            Source::CaptureCard(_) => true,
            Source::AudioDevice(_) => true,
        }
    }
}

/// RTMP input source - incoming RTMP stream
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RtmpSource {
    /// Unique identifier
    pub id: String,
    /// User-friendly name
    pub name: String,
    /// Network interface to bind to (e.g., "0.0.0.0", "127.0.0.1")
    pub bind_address: String,
    /// TCP port to listen on (e.g., 1935)
    pub port: u16,
    /// RTMP application/path (e.g., "live", "ingest")
    pub application: String,
}

impl Default for RtmpSource {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: "RTMP Input".to_string(),
            bind_address: "0.0.0.0".to_string(),
            port: 1935,
            application: "live".to_string(),
        }
    }
}

/// Media file source - local video/audio file
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaFileSource {
    /// Unique identifier
    pub id: String,
    /// User-friendly name
    pub name: String,
    /// Absolute path to the media file
    pub file_path: String,
    /// Whether to loop playback
    pub loop_playback: bool,
    /// Whether this is an audio-only file
    #[serde(default)]
    pub audio_only: bool,
}

/// Screen capture source - captures a display
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenCaptureSource {
    /// Unique identifier
    pub id: String,
    /// User-friendly name
    pub name: String,
    /// Platform-specific display identifier
    pub display_id: String,
    /// Whether to capture the cursor
    #[serde(default = "default_true")]
    pub capture_cursor: bool,
    /// Whether to capture desktop audio (macOS/Windows)
    #[serde(default)]
    pub capture_audio: bool,
    /// Target frame rate for capture
    #[serde(default = "default_fps")]
    pub fps: u32,
}

fn default_true() -> bool {
    true
}

fn default_fps() -> u32 {
    30
}

/// Camera source - webcam or video capture device
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraSource {
    /// Unique identifier
    pub id: String,
    /// User-friendly name
    pub name: String,
    /// Platform-specific device ID
    pub device_id: String,
    /// Requested width (None = device default)
    pub width: Option<u32>,
    /// Requested height (None = device default)
    pub height: Option<u32>,
    /// Requested frame rate (None = device default)
    pub fps: Option<u32>,
}

/// Capture card source - HDMI/SDI capture devices
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureCardSource {
    /// Unique identifier
    pub id: String,
    /// User-friendly name
    pub name: String,
    /// Platform-specific device ID (e.g., "Elgato HD60 S")
    pub device_id: String,
    /// Input format (e.g., "hdmi", "component", "sdi")
    pub input_format: Option<String>,
}

/// Audio device source - microphone, line-in, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDeviceSource {
    /// Unique identifier
    pub id: String,
    /// User-friendly name
    pub name: String,
    /// Platform-specific device ID
    pub device_id: String,
    /// Number of channels (None = device default)
    pub channels: Option<u8>,
    /// Sample rate in Hz (None = device default)
    pub sample_rate: Option<u32>,
}

// Device discovery result types

/// Discovered camera device
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraDevice {
    pub device_id: String,
    pub name: String,
    pub resolutions: Vec<Resolution>,
}

/// Available resolution for a device
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
    pub fps: Vec<u32>,
}

/// Discovered display for screen capture
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayInfo {
    pub display_id: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
}

/// Discovered audio input device
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioInputDevice {
    pub device_id: String,
    pub name: String,
    pub channels: u8,
    pub sample_rate: u32,
    pub is_default: bool,
}

/// Discovered capture card device
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureCardDevice {
    pub device_id: String,
    pub name: String,
    pub inputs: Vec<String>,
}
