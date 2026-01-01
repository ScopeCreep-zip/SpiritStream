// Encoders Model
// Available video and audio encoders

use serde::{Deserialize, Serialize};

/// Available encoders detected on the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Encoders {
    /// Available video encoders
    pub video: Vec<String>,

    /// Available audio encoders
    pub audio: Vec<String>,
}

impl Default for Encoders {
    fn default() -> Self {
        Self {
            video: vec!["libx264".to_string()],
            audio: vec!["aac".to_string()],
        }
    }
}
