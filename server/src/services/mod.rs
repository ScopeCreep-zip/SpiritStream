// SpiritStream Services
// Business logic layer

mod profile_manager;
mod ffmpeg_handler;
mod ffmpeg_downloader;
mod encryption;
mod settings_manager;
mod theme_manager;
mod embedded_themes;
mod platform_registry;
mod log_manager;
mod events;
mod path_validator;
mod device_discovery;
mod compositor;
mod preview_handler;
mod screen_capture;
mod audio_capture;
mod camera_capture;
mod native_preview;
mod recording_service;
mod replay_buffer;
mod capture_indicator;
mod permissions;
mod go2rtc_client;
mod go2rtc_manager;
mod capture_frame;
mod h264_capture;
mod audio_levels;
mod audio_level_extractor;

// macOS-specific ScreenCaptureKit audio capture
#[cfg(target_os = "macos")]
mod sck_audio_capture;
#[cfg(target_os = "macos")]
pub use sck_audio_capture::*;

pub use profile_manager::*;
pub use ffmpeg_handler::*;
pub use ffmpeg_downloader::*;
pub use encryption::*;
pub use settings_manager::*;
pub use theme_manager::*;
pub use embedded_themes::{get_embedded_theme_tokens, get_embedded_theme_list, is_embedded_theme};
pub use platform_registry::*;
pub use log_manager::*;
pub use events::*;
pub use path_validator::*;
pub use device_discovery::*;
pub use compositor::*;
pub use preview_handler::*;
pub use screen_capture::*;
pub use audio_capture::*;
pub use camera_capture::*;
pub use native_preview::*;
pub use recording_service::*;
pub use replay_buffer::*;
pub use capture_indicator::*;
pub use permissions::*;
pub use go2rtc_client::*;
pub use go2rtc_manager::*;
pub use capture_frame::*;
pub use h264_capture::*;
pub use audio_levels::*;
pub use audio_level_extractor::*;
