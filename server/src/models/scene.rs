// Scene Model
// Represents composition of sources with layout and audio mixing

use serde::{Deserialize, Serialize};

/// Scene transition types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransitionType {
    Cut,
    Fade,
    Crossfade,
    SlideLeft,
    SlideRight,
    SlideUp,
    SlideDown,
    WipeLeft,
    WipeRight,
    WipeUp,
    WipeDown,
}

impl Default for TransitionType {
    fn default() -> Self {
        Self::Fade
    }
}

/// Scene transition configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneTransition {
    /// Transition type
    #[serde(rename = "type")]
    pub transition_type: TransitionType,
    /// Duration in milliseconds (100-2000)
    pub duration_ms: u32,
}

impl Default for SceneTransition {
    fn default() -> Self {
        Self {
            transition_type: TransitionType::Fade,
            duration_ms: 300,
        }
    }
}

/// Scene - composition of sources with layout and audio mixing
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    /// Unique identifier
    pub id: String,
    /// User-friendly name
    pub name: String,
    /// Canvas width in pixels (e.g., 1920)
    pub canvas_width: u32,
    /// Canvas height in pixels (e.g., 1080)
    pub canvas_height: u32,
    /// Layers in this scene (ordered by z_index)
    #[serde(default)]
    pub layers: Vec<SourceLayer>,
    /// Audio mixer configuration
    #[serde(default)]
    pub audio_mixer: AudioMixer,
    /// Override transition for when switching TO this scene
    #[serde(default)]
    pub transition_in: Option<SceneTransition>,
}

impl Scene {
    /// Create a new scene with default settings
    pub fn new(name: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            canvas_width: 1920,
            canvas_height: 1080,
            layers: Vec::new(),
            audio_mixer: AudioMixer::default(),
            transition_in: None,
        }
    }

    /// Create a scene with a single fullscreen source
    pub fn with_fullscreen_source(name: &str, source_id: &str) -> Self {
        let mut scene = Self::new(name);
        scene.layers.push(SourceLayer::fullscreen(source_id, 1920, 1080));
        scene.audio_mixer.tracks.push(AudioTrack::new(source_id));
        scene
    }

    /// Get layers sorted by z_index (lowest first = bottom of stack)
    pub fn sorted_layers(&self) -> Vec<&SourceLayer> {
        let mut sorted: Vec<_> = self.layers.iter().collect();
        sorted.sort_by_key(|l| l.z_index);
        sorted
    }

    /// Add a layer to the scene
    pub fn add_layer(&mut self, source_id: &str, transform: Transform) -> String {
        let layer_id = uuid::Uuid::new_v4().to_string();
        let z_index = self.layers.iter().map(|l| l.z_index).max().unwrap_or(0) + 1;

        self.layers.push(SourceLayer {
            id: layer_id.clone(),
            source_id: source_id.to_string(),
            visible: true,
            locked: false,
            transform,
            z_index,
        });

        // Add audio track if not already present
        if !self.audio_mixer.tracks.iter().any(|t| t.source_id == source_id) {
            self.audio_mixer.tracks.push(AudioTrack::new(source_id));
        }

        layer_id
    }

    /// Remove a layer from the scene
    pub fn remove_layer(&mut self, layer_id: &str) -> Option<SourceLayer> {
        let pos = self.layers.iter().position(|l| l.id == layer_id)?;
        Some(self.layers.remove(pos))
    }
}

/// SourceLayer - a source instance positioned within a scene
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceLayer {
    /// Unique layer ID (unique within scene)
    pub id: String,
    /// Reference to the source ID
    pub source_id: String,
    /// Whether this layer is visible
    #[serde(default = "default_true")]
    pub visible: bool,
    /// Whether this layer is locked (can't be moved/resized)
    #[serde(default)]
    pub locked: bool,
    /// Position and size transformation
    pub transform: Transform,
    /// Stacking order (higher = on top)
    #[serde(default)]
    pub z_index: i32,
}

fn default_true() -> bool {
    true
}

impl SourceLayer {
    /// Create a fullscreen layer for a source
    pub fn fullscreen(source_id: &str, canvas_width: u32, canvas_height: u32) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            source_id: source_id.to_string(),
            visible: true,
            locked: false,
            transform: Transform {
                x: 0,
                y: 0,
                width: canvas_width,
                height: canvas_height,
                rotation: 0.0,
                crop: None,
            },
            z_index: 0,
        }
    }
}

/// Transform - position and size of a layer on the canvas
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transform {
    /// X position from left edge of canvas
    pub x: i32,
    /// Y position from top edge of canvas
    pub y: i32,
    /// Rendered width in pixels
    pub width: u32,
    /// Rendered height in pixels
    pub height: u32,
    /// Rotation in degrees (0-360)
    #[serde(default)]
    pub rotation: f32,
    /// Optional cropping
    pub crop: Option<Crop>,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            rotation: 0.0,
            crop: None,
        }
    }
}

/// Crop - pixels to remove from source edges
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Crop {
    pub top: u32,
    pub bottom: u32,
    pub left: u32,
    pub right: u32,
}

/// AudioMixer - audio mixing configuration for a scene
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioMixer {
    /// Master volume (0.0 - 2.0, 1.0 = unity gain)
    #[serde(default = "default_volume")]
    pub master_volume: f32,
    /// Master muted state
    #[serde(default)]
    pub master_muted: bool,
    /// Individual audio tracks
    #[serde(default)]
    pub tracks: Vec<AudioTrack>,
}

fn default_volume() -> f32 {
    1.0
}

impl Default for AudioMixer {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            master_muted: false,
            tracks: Vec::new(),
        }
    }
}

impl AudioMixer {
    /// Create a mixer with a single source
    pub fn with_source(source_id: &str) -> Self {
        Self {
            master_volume: 1.0,
            master_muted: false,
            tracks: vec![AudioTrack::new(source_id)],
        }
    }
}

/// AudioTrack - audio settings for a single source in the mixer
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTrack {
    /// Reference to the source ID
    pub source_id: String,
    /// Volume (0.0 - 2.0, 1.0 = unity gain)
    #[serde(default = "default_volume")]
    pub volume: f32,
    /// Whether this track is muted
    #[serde(default)]
    pub muted: bool,
    /// Whether this track is soloed (mutes all others)
    #[serde(default)]
    pub solo: bool,
}

impl AudioTrack {
    /// Create a new audio track with default settings
    pub fn new(source_id: &str) -> Self {
        Self {
            source_id: source_id.to_string(),
            volume: 1.0,
            muted: false,
            solo: false,
        }
    }
}
