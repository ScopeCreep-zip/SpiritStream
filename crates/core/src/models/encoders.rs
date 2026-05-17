// Encoders Model
// Available video and audio encoders

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

/// Hardware vs software vs passthrough classification. Authoritative
/// source for the encoder-card badge; the frontend no longer keeps a
/// hard-coded `HARDWARE_ENCODERS` Set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub enum EncoderKind {
    Hardware,
    Software,
    Passthrough,
}

/// Per-encoder metadata: classification + preset-family key. The
/// `family` value matches keys in `EncoderPresetsResponse.presets`
/// (`libx264`, `libx265`, `nvenc`, `amf`, `qsv`, `videotoolbox`,
/// `vaapi`). Empty for passthrough / audio-only encoders.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct EncoderMeta {
    pub kind: EncoderKind,
    pub family: String,
}

/// Available encoders detected on the system. `metadata` carries the
/// per-encoder classification + preset family so the frontend never
/// substring-matches codec names to figure out "is this hardware?" or
/// "which preset list applies?".
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct Encoders {
    pub video: Vec<String>,
    pub audio: Vec<String>,
    pub metadata: HashMap<String, EncoderMeta>,
}

impl Default for Encoders {
    fn default() -> Self {
        let mut metadata = HashMap::new();
        metadata.insert(
            "libx264".to_string(),
            EncoderMeta {
                kind: EncoderKind::Software,
                family: "libx264".to_string(),
            },
        );
        metadata.insert(
            "aac".to_string(),
            EncoderMeta {
                kind: EncoderKind::Software,
                family: String::new(),
            },
        );
        Self {
            video: vec!["libx264".to_string()],
            audio: vec!["aac".to_string()],
            metadata,
        }
    }
}
