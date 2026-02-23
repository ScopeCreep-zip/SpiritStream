// Audio Engine Types
// Core configuration and constants for the audio mixing engine

use crate::models::SpeakerLayout;
use serde::{Deserialize, Serialize};

/// Maximum number of output audio tracks (OBS supports 6)
pub const MAX_AUDIO_TRACKS: usize = 6;

/// Default ring buffer capacity in samples per channel
/// ~170ms at 48kHz stereo — enough to absorb jitter without excess latency
pub const RING_BUFFER_CAPACITY: usize = 16384;

/// Mixer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixerConfig {
    /// Output sample rate (Hz)
    pub sample_rate: u32,
    /// Speaker/channel layout
    pub speaker_layout: SpeakerLayout,
    /// Samples per mixer callback (determines latency: 1024/48000 = ~21.3ms)
    pub samples_per_callback: usize,
}

impl Default for MixerConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            speaker_layout: SpeakerLayout::Stereo,
            samples_per_callback: 1024,
        }
    }
}

impl MixerConfig {
    /// Number of channels for the current speaker layout
    pub fn channels(&self) -> usize {
        match self.speaker_layout {
            SpeakerLayout::Mono => 1,
            SpeakerLayout::Stereo => 2,
            SpeakerLayout::FivePointOne => 6,
            SpeakerLayout::SevenPointOne => 8,
        }
    }
}
