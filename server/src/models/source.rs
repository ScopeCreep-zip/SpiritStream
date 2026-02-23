// Source Model
// Represents different input source types for multi-input streaming
//
// Uses SourceBase trait to eliminate repeated 14-arm match statements.
// Each variant's inner struct implements SourceBase for common accessors.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Common trait — eliminates N-arm matches for id/name access
// ---------------------------------------------------------------------------

/// Common accessors shared by all source types.
/// Implementing this trait for each source variant's inner struct lets us
/// delegate `Source::id()` and `Source::name()` through a single match + trait call.
pub trait SourceBase {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
}

/// Macro to implement SourceBase for a struct with `id: String` and `name: String` fields
macro_rules! impl_source_base {
    ($($t:ty),+ $(,)?) => {
        $(
            impl SourceBase for $t {
                fn id(&self) -> &str { &self.id }
                fn name(&self) -> &str { &self.name }
            }
        )+
    };
}

// ---------------------------------------------------------------------------
// Source capabilities — declarative metadata for each source type
// ---------------------------------------------------------------------------

/// Which backend capture service handles this source type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureBackend {
    /// Screen/display capture via scap
    Screen,
    /// Camera capture via FFmpeg
    Camera,
    /// Audio-only capture via cpal
    Audio,
    /// FFmpeg-based capture (media files, RTMP, capture cards, NDI)
    FFmpeg,
    /// Rendered client-side in the browser (color, text, browser, nested scene)
    ClientSide,
}

/// Declarative capabilities for a source type
#[derive(Debug, Clone)]
pub struct SourceCapabilities {
    pub has_video: bool,
    pub has_audio: bool,
    pub needs_device: bool,
    pub capture_backend: CaptureBackend,
}

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
    /// Window capture (specific application window)
    WindowCapture(WindowCaptureSource),
    /// Camera/webcam device
    Camera(CameraSource),
    /// Capture card (HDMI capture devices like Elgato)
    CaptureCard(CaptureCardSource),
    /// Audio-only input device (microphone, line-in)
    AudioDevice(AudioDeviceSource),
    /// Solid color fill
    Color(ColorSource),
    /// Text overlay with styling
    Text(TextSource),
    /// Web page iframe (browser source)
    Browser(BrowserSource),
    /// Media playlist (multiple files in sequence)
    MediaPlaylist(MediaPlaylistSource),
    /// Nested scene (embeds another scene)
    NestedScene(NestedSceneSource),
    /// Game capture (hardware-accelerated game capture)
    GameCapture(GameCaptureSource),
    /// NDI source (network video over NDI protocol)
    Ndi(NdiSource),
}

// Implement SourceBase for all 14 source structs via macro
impl_source_base!(
    RtmpSource,
    MediaFileSource,
    ScreenCaptureSource,
    WindowCaptureSource,
    CameraSource,
    CaptureCardSource,
    AudioDeviceSource,
    ColorSource,
    TextSource,
    BrowserSource,
    MediaPlaylistSource,
    NestedSceneSource,
    GameCaptureSource,
    NdiSource,
);

impl Source {
    /// Access the inner source as a trait object for common field access
    fn base(&self) -> &dyn SourceBase {
        match self {
            Source::Rtmp(s) => s,
            Source::MediaFile(s) => s,
            Source::ScreenCapture(s) => s,
            Source::WindowCapture(s) => s,
            Source::Camera(s) => s,
            Source::CaptureCard(s) => s,
            Source::AudioDevice(s) => s,
            Source::Color(s) => s,
            Source::Text(s) => s,
            Source::Browser(s) => s,
            Source::MediaPlaylist(s) => s,
            Source::NestedScene(s) => s,
            Source::GameCapture(s) => s,
            Source::Ndi(s) => s,
        }
    }

    /// Get the unique ID of this source
    pub fn id(&self) -> &str {
        self.base().id()
    }

    /// Get the user-friendly name of this source
    pub fn name(&self) -> &str {
        self.base().name()
    }

    /// Get declarative capabilities for this source type
    pub fn capabilities(&self) -> SourceCapabilities {
        match self {
            Source::Rtmp(_) =>        SourceCapabilities { has_video: true,  has_audio: true,  needs_device: false, capture_backend: CaptureBackend::FFmpeg },
            Source::MediaFile(s) =>   SourceCapabilities { has_video: !s.audio_only, has_audio: true, needs_device: false, capture_backend: CaptureBackend::FFmpeg },
            Source::ScreenCapture(_) => SourceCapabilities { has_video: true, has_audio: false, needs_device: true, capture_backend: CaptureBackend::Screen },
            Source::WindowCapture(_) => SourceCapabilities { has_video: true, has_audio: false, needs_device: true, capture_backend: CaptureBackend::Screen },
            Source::Camera(_) =>      SourceCapabilities { has_video: true,  has_audio: false, needs_device: true,  capture_backend: CaptureBackend::Camera },
            Source::CaptureCard(_) => SourceCapabilities { has_video: true,  has_audio: true,  needs_device: true,  capture_backend: CaptureBackend::FFmpeg },
            Source::AudioDevice(_) => SourceCapabilities { has_video: false, has_audio: true,  needs_device: true,  capture_backend: CaptureBackend::Audio },
            Source::Color(_) =>       SourceCapabilities { has_video: true,  has_audio: false, needs_device: false, capture_backend: CaptureBackend::ClientSide },
            Source::Text(_) =>        SourceCapabilities { has_video: true,  has_audio: false, needs_device: false, capture_backend: CaptureBackend::ClientSide },
            Source::Browser(_) =>     SourceCapabilities { has_video: true,  has_audio: false, needs_device: false, capture_backend: CaptureBackend::ClientSide },
            Source::MediaPlaylist(_) => SourceCapabilities { has_video: true, has_audio: true, needs_device: false, capture_backend: CaptureBackend::FFmpeg },
            Source::NestedScene(_) => SourceCapabilities { has_video: true,  has_audio: false, needs_device: false, capture_backend: CaptureBackend::ClientSide },
            Source::GameCapture(_) => SourceCapabilities { has_video: true,  has_audio: false, needs_device: false, capture_backend: CaptureBackend::Screen },
            Source::Ndi(_) =>         SourceCapabilities { has_video: true,  has_audio: true,  needs_device: false, capture_backend: CaptureBackend::FFmpeg },
        }
    }

    /// Check if this source has video output
    pub fn has_video(&self) -> bool {
        self.capabilities().has_video
    }

    /// Check if this source has audio output
    /// Note: Camera returns false because audio comes from the auto-created linked AudioDeviceSource.
    /// For dynamic checks (e.g., captureAudio toggle), use the instance method below.
    pub fn has_audio(&self) -> bool {
        match self {
            Source::Rtmp(s) => s.capture_audio,
            Source::MediaFile(s) => s.capture_audio,
            Source::ScreenCapture(s) => s.capture_audio,
            Source::WindowCapture(s) => s.capture_audio,
            Source::Camera(_) => false,
            Source::CaptureCard(s) => s.capture_audio,
            Source::AudioDevice(_) => true,
            Source::Color(_) => false,
            Source::Text(_) => false,
            Source::Browser(_) => false,
            Source::MediaPlaylist(s) => s.capture_audio,
            Source::NestedScene(_) => false,
            Source::GameCapture(s) => s.capture_audio,
            Source::Ndi(s) => s.capture_audio,
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
    /// Whether to capture audio from this source
    #[serde(default = "default_true")]
    pub capture_audio: bool,
}

impl Default for RtmpSource {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: "RTMP Input".to_string(),
            bind_address: "0.0.0.0".to_string(),
            port: 1935,
            application: "live".to_string(),
            capture_audio: true,
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
    /// Whether to capture audio from this media file (default: true)
    #[serde(default = "default_true")]
    pub capture_audio: bool,
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
    /// The actual device name as reported by the OS (e.g., "Capture screen 0" on macOS)
    /// Used for go2rtc registration
    #[serde(default)]
    pub device_name: Option<String>,
    /// Whether to capture the cursor
    #[serde(default = "default_true")]
    pub capture_cursor: bool,
    /// Whether to capture desktop audio (macOS/Windows)
    #[serde(default)]
    pub capture_audio: bool,
    /// Target frame rate for capture
    #[serde(default = "default_fps")]
    pub fps: u32,
    /// Target capture resolution preset. Controls scap output resolution.
    /// Default "1080p" balances quality and performance. "captured" uses native display resolution.
    #[serde(default = "default_capture_resolution")]
    pub capture_resolution: String,
}

/// Window capture source - captures a specific application window
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowCaptureSource {
    /// Unique identifier
    pub id: String,
    /// User-friendly name
    pub name: String,
    /// Platform-specific window identifier
    pub window_id: String,
    /// Window title at time of selection
    pub window_title: String,
    /// Process name or app name
    #[serde(default)]
    pub process_name: Option<String>,
    /// Whether to capture the cursor
    #[serde(default = "default_true")]
    pub capture_cursor: bool,
    /// Target frame rate for capture
    #[serde(default = "default_fps")]
    pub fps: u32,
    /// Whether to capture window audio (macOS/Windows)
    #[serde(default)]
    pub capture_audio: bool,
    /// Target capture resolution preset. Controls scap output resolution.
    /// Default "1080p" balances quality and performance. "captured" uses native display resolution.
    #[serde(default = "default_capture_resolution")]
    pub capture_resolution: String,
}

fn default_capture_resolution() -> String {
    "1080p".to_string()
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
    /// Whether to capture audio from built-in microphone
    #[serde(default)]
    pub capture_audio: bool,
    /// Auto-discovered linked audio device ID (from CameraDevice)
    /// When capture_audio is true, an AudioDeviceSource will be auto-created for this device
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linked_audio_device_id: Option<String>,
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
    /// Whether to capture audio from this source
    #[serde(default = "default_true")]
    pub capture_audio: bool,
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
    /// If this was auto-created as linked audio for another source (e.g., camera)
    /// When the parent source is deleted, this source should also be deleted
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linked_to_source_id: Option<String>,
}

// Device discovery result types

/// Discovered camera device
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraDevice {
    pub device_id: String,
    pub name: String,
    pub resolutions: Vec<Resolution>,
    /// Auto-discovered linked audio device ID (e.g., camera's built-in microphone)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linked_audio_device_id: Option<String>,
    /// Name of the linked audio device
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linked_audio_device_name: Option<String>,
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
    /// The actual device name as reported by the OS (e.g., "Capture screen 0" on macOS)
    /// Used for go2rtc registration
    #[serde(default)]
    pub device_name: Option<String>,
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

// Client-side rendered sources (no go2rtc/WebRTC needed)

/// Color source - solid color fill
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorSource {
    /// Unique identifier
    pub id: String,
    /// User-friendly name
    pub name: String,
    /// Hex color string (e.g., "#7C3AED")
    pub color: String,
    /// Opacity (0.0 - 1.0)
    pub opacity: f32,
}

/// Text outline configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextOutline {
    pub enabled: bool,
    pub color: String,
    pub width: u32,
}

/// Text source - text overlay with styling
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSource {
    /// Unique identifier
    pub id: String,
    /// User-friendly name
    pub name: String,
    /// Text content (supports multiline)
    pub content: String,
    /// Font family name
    pub font_family: String,
    /// Font size in pixels
    pub font_size: u32,
    /// Font weight
    pub font_weight: String, // "normal" | "bold"
    /// Font style
    pub font_style: String, // "normal" | "italic"
    /// Text color (hex)
    pub text_color: String,
    /// Optional background color (hex)
    #[serde(default)]
    pub background_color: Option<String>,
    /// Background opacity (0.0 - 1.0)
    #[serde(default = "default_background_opacity")]
    pub background_opacity: f32,
    /// Text alignment
    pub text_align: String, // "left" | "center" | "right"
    /// Line height multiplier
    #[serde(default = "default_line_height")]
    pub line_height: f32,
    /// Padding in pixels
    #[serde(default = "default_padding")]
    pub padding: u32,
    /// Optional text outline
    #[serde(default)]
    pub outline: Option<TextOutline>,
}

fn default_background_opacity() -> f32 {
    0.8
}

fn default_line_height() -> f32 {
    1.2
}

fn default_padding() -> u32 {
    16
}

/// Browser source - web page iframe
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserSource {
    /// Unique identifier
    pub id: String,
    /// User-friendly name
    pub name: String,
    /// URL to load
    pub url: String,
    /// Viewport width (default: 1920)
    #[serde(default = "default_browser_width")]
    pub width: u32,
    /// Viewport height (default: 1080)
    #[serde(default = "default_browser_height")]
    pub height: u32,
    /// Optional custom CSS to inject
    #[serde(default)]
    pub custom_css: Option<String>,
    /// Auto-refresh interval in seconds (0 = manual only)
    #[serde(default)]
    pub refresh_interval: Option<u32>,
    /// Token that changes to trigger manual refresh
    #[serde(default)]
    pub refresh_token: Option<String>,
}

fn default_browser_width() -> u32 {
    1920
}

fn default_browser_height() -> u32 {
    1080
}

/// Media playlist source - plays multiple media files in sequence
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaPlaylistSource {
    /// Unique identifier
    pub id: String,
    /// User-friendly name
    pub name: String,
    /// Playlist items
    pub items: Vec<PlaylistItem>,
    /// Currently playing item index
    #[serde(default)]
    pub current_item_index: usize,
    /// Whether to auto-advance to next item
    #[serde(default = "default_true")]
    pub auto_advance: bool,
    /// Shuffle mode
    #[serde(default)]
    pub shuffle_mode: ShuffleMode,
    /// Whether to fade between items
    #[serde(default)]
    pub fade_between_items: bool,
    /// Fade duration in milliseconds
    #[serde(default = "default_fade_duration")]
    pub fade_duration_ms: u32,
    /// Whether to capture audio from playlist items
    #[serde(default = "default_true")]
    pub capture_audio: bool,
}

fn default_fade_duration() -> u32 {
    500
}

/// Playlist item
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistItem {
    /// Unique identifier
    pub id: String,
    /// File path
    pub file_path: String,
    /// Duration in seconds (auto-detected)
    #[serde(default)]
    pub duration: Option<f64>,
    /// Display name (defaults to filename)
    #[serde(default)]
    pub name: Option<String>,
}

/// Shuffle mode for playlists
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShuffleMode {
    #[default]
    None,
    All,
    RepeatOne,
}

/// Nested scene source - embeds another scene
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NestedSceneSource {
    /// Unique identifier
    pub id: String,
    /// User-friendly name
    pub name: String,
    /// ID of the scene to embed
    pub referenced_scene_id: String,
}

/// Window info for window discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    /// Platform-specific window identifier
    pub window_id: String,
    /// Window title
    pub title: String,
    /// Process name
    #[serde(default)]
    pub process_name: Option<String>,
    /// Application name
    #[serde(default)]
    pub app_name: Option<String>,
    /// Window width
    #[serde(default)]
    pub width: Option<u32>,
    /// Window height
    #[serde(default)]
    pub height: Option<u32>,
}

/// Game capture source - hardware-accelerated game capture
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameCaptureSource {
    /// Unique identifier
    pub id: String,
    /// User-friendly name
    pub name: String,
    /// Target type: "any" captures any fullscreen game, "specific" targets a window/process
    #[serde(default = "default_target_type")]
    pub target_type: String,
    /// Window title to capture (when target_type is "specific")
    #[serde(default)]
    pub window_title: Option<String>,
    /// Process name to capture (when target_type is "specific")
    #[serde(default)]
    pub process_name: Option<String>,
    /// Capture method: "auto", "bitblt", "dxgi", "opengl"
    #[serde(default = "default_capture_mode")]
    pub capture_mode: String,
    /// Whether to include cursor in capture
    #[serde(default)]
    pub capture_cursor: bool,
    /// Enable anti-cheat compatible hooking
    #[serde(default)]
    pub anti_cheat_hook: bool,
    /// Target frame rate
    #[serde(default = "default_game_fps")]
    pub fps: u32,
    /// Whether to capture game audio
    #[serde(default)]
    pub capture_audio: bool,
}

fn default_target_type() -> String {
    "any".to_string()
}

fn default_capture_mode() -> String {
    "auto".to_string()
}

fn default_game_fps() -> u32 {
    60
}

/// NDI source - receives video over network via NDI protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NdiSource {
    /// Unique identifier
    pub id: String,
    /// User-friendly name
    pub name: String,
    /// Name of the NDI source to receive
    pub source_name: String,
    /// Optional specific IP address (auto-discovers if not set)
    #[serde(default)]
    pub ip_address: Option<String>,
    /// Use low bandwidth mode (lower quality, less network usage)
    #[serde(default)]
    pub low_bandwidth: bool,
    /// Name to identify this receiver on the network
    #[serde(default = "default_receiver_name")]
    pub receiver_name: String,
    /// Whether to capture audio from this source
    #[serde(default = "default_true")]
    pub capture_audio: bool,
}

fn default_receiver_name() -> String {
    "SpiritStream".to_string()
}

// ---------------------------------------------------------------------------
// Source-level audio configuration (OBS pattern: audio config lives on source,
// not per-scene. obs_source_set_volume() applies globally.)
// ---------------------------------------------------------------------------

/// Audio monitoring mode — how audio is routed for preview listening
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MonitoringType {
    /// Audio goes to stream output only (default)
    None,
    /// Audio plays through monitoring device only, excluded from stream
    MonitorOnly,
    /// Audio plays through both monitoring device and stream output
    MonitorAndOutput,
}

impl Default for MonitoringType {
    fn default() -> Self {
        Self::None
    }
}

/// Fader curve type — how the volume fader maps to gain
/// OBS supports cubic (default), linear, and sine curves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FaderCurve {
    /// Cubic curve (default) — perceptually uniform, OBS default
    Cubic,
    /// Linear curve — direct 1:1 mapping
    Linear,
    /// Sine curve — gentle taper at extremes
    Sine,
}

impl Default for FaderCurve {
    fn default() -> Self {
        Self::Cubic
    }
}

/// Speaker/channel layout for audio processing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpeakerLayout {
    Mono,
    Stereo,
    FivePointOne,
    SevenPointOne,
}

impl Default for SpeakerLayout {
    fn default() -> Self {
        Self::Stereo
    }
}

/// Per-source audio configuration (OBS parity: source-level, not scene-level)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceAudioConfig {
    /// Volume multiplier (0.0-20.0, 1.0 = unity gain). OBS range.
    #[serde(default = "default_unity")]
    pub volume: f32,
    /// Whether this source's audio is muted
    #[serde(default)]
    pub muted: bool,
    /// Whether this source is soloed (mutes all non-soloed sources)
    #[serde(default)]
    pub solo: bool,
    /// Sync offset in milliseconds (positive = delay audio)
    #[serde(default)]
    pub sync_offset_ms: i64,
    /// How audio is routed for monitoring
    #[serde(default)]
    pub monitoring_type: MonitoringType,
    /// Bitmask of which output tracks receive this source's audio (bits 0-5 = tracks 1-6)
    #[serde(default = "default_track_bitmask")]
    pub track_bitmask: u8,
    /// Stereo balance (-1.0 = full left, 0.0 = center, 1.0 = full right)
    #[serde(default)]
    pub balance: f32,
    /// Fader curve type (how volume fader maps to gain)
    #[serde(default)]
    pub fader_curve: FaderCurve,
    /// Audio filter chain applied in order
    #[serde(default)]
    pub audio_filters: Vec<AudioFilterConfig>,
    /// Config version — incremented by frontend on any audio config change.
    /// Mixer thread uses this to detect filter parameter changes (not just count changes).
    #[serde(default)]
    pub config_version: u64,
}

fn default_unity() -> f32 {
    1.0
}

fn default_track_bitmask() -> u8 {
    0b000001 // Track 1 only
}

impl Default for SourceAudioConfig {
    fn default() -> Self {
        Self {
            volume: 1.0,
            muted: false,
            solo: false,
            sync_offset_ms: 0,
            monitoring_type: MonitoringType::None,
            track_bitmask: 0b000001,
            balance: 0.0,
            fader_curve: FaderCurve::default(),
            audio_filters: Vec::new(),
            config_version: 0,
        }
    }
}

/// Audio filter configuration — serialized as tagged enum for JSON interop
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AudioFilterConfig {
    Gain {
        #[serde(default)]
        gain_db: f32,
    },
    Compressor {
        #[serde(default = "default_threshold")]
        threshold_db: f32,
        #[serde(default = "default_ratio")]
        ratio: f32,
        #[serde(default = "default_attack")]
        attack_ms: f32,
        #[serde(default = "default_release")]
        release_ms: f32,
        #[serde(default)]
        output_gain_db: f32,
        #[serde(default)]
        sidechain_source_id: Option<String>,
    },
    NoiseGate {
        #[serde(default = "default_open_threshold")]
        open_threshold_db: f32,
        #[serde(default = "default_close_threshold")]
        close_threshold_db: f32,
        #[serde(default = "default_attack")]
        attack_ms: f32,
        #[serde(default = "default_hold")]
        hold_ms: f32,
        #[serde(default = "default_release")]
        release_ms: f32,
    },
    Expander {
        #[serde(default = "default_threshold")]
        threshold_db: f32,
        #[serde(default = "default_ratio")]
        ratio: f32,
        #[serde(default = "default_attack")]
        attack_ms: f32,
        #[serde(default = "default_release")]
        release_ms: f32,
    },
    Limiter {
        #[serde(default = "default_limiter_threshold")]
        threshold_db: f32,
        #[serde(default = "default_release")]
        release_ms: f32,
    },
    NoiseSuppression {
        /// Suppression level (0.0-1.0, higher = more aggressive)
        #[serde(default = "default_suppress_level")]
        suppress_level: f32,
    },
}

fn default_threshold() -> f32 { -18.0 }
fn default_ratio() -> f32 { 4.0 }
fn default_attack() -> f32 { 6.0 }
fn default_release() -> f32 { 60.0 }
fn default_open_threshold() -> f32 { -26.0 }
fn default_close_threshold() -> f32 { -32.0 }
fn default_hold() -> f32 { 200.0 }
fn default_limiter_threshold() -> f32 { -0.5 }
fn default_suppress_level() -> f32 { 0.5 }
