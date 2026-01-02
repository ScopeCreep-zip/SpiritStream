// Settings Model
// Application-wide configuration

use serde::{Deserialize, Serialize};

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    // General
    pub language: String,
    pub start_minimized: bool,
    pub show_notifications: bool,

    // FFmpeg
    pub ffmpeg_path: String,
    pub auto_download_ffmpeg: bool,

    // Data & Privacy
    pub encrypt_stream_keys: bool,

    // Last used profile
    pub last_profile: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            start_minimized: false,
            show_notifications: true,
            ffmpeg_path: String::new(),
            auto_download_ffmpeg: true,
            encrypt_stream_keys: false,
            last_profile: None,
        }
    }
}
