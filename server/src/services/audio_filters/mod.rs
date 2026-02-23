// Audio Filter Chain
// Per-source DSP processing pipeline. Each filter implements the AudioFilter trait.
// FilterChain runs filters in order on interleaved f32 samples.

pub mod gain;
pub mod compressor;
pub mod noise_gate;
pub mod expander;
pub mod limiter;
pub mod noise_suppression;

use crate::models::AudioFilterConfig;

/// Trait for real-time audio filters.
/// All filters process interleaved f32 samples in-place.
pub trait AudioFilter: Send {
    fn process(&mut self, samples: &mut [f32], channels: usize, sample_rate: u32);
    fn reset(&mut self);
    fn filter_type(&self) -> &str;
}

/// Ordered chain of audio filters applied to a single source.
pub struct FilterChain {
    filters: Vec<Box<dyn AudioFilter>>,
}

impl FilterChain {
    pub fn new() -> Self {
        Self { filters: Vec::new() }
    }

    /// Build a filter chain from serialized configs.
    pub fn from_configs(configs: &[AudioFilterConfig]) -> Self {
        let filters: Vec<Box<dyn AudioFilter>> = configs
            .iter()
            .filter_map(|cfg| build_filter(cfg))
            .collect();

        log::debug!(
            "[FilterChain] Built chain with {} filters: [{}]",
            filters.len(),
            filters.iter().map(|f| f.filter_type()).collect::<Vec<_>>().join(", ")
        );

        Self { filters }
    }

    /// Process samples through the entire chain (in-place).
    pub fn process(&mut self, samples: &mut [f32], channels: usize, sample_rate: u32) {
        for filter in &mut self.filters {
            filter.process(samples, channels, sample_rate);
        }
    }

    /// True if the chain has zero filters (passthrough).
    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }

    pub fn len(&self) -> usize {
        self.filters.len()
    }
}

/// Construct a boxed filter from a config variant.
fn build_filter(cfg: &AudioFilterConfig) -> Option<Box<dyn AudioFilter>> {
    match cfg {
        AudioFilterConfig::Gain { gain_db } => {
            Some(Box::new(gain::GainFilter::new(*gain_db)))
        }
        AudioFilterConfig::Compressor {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            output_gain_db,
            ..
        } => {
            Some(Box::new(compressor::CompressorFilter::new(
                *threshold_db,
                *ratio,
                *attack_ms,
                *release_ms,
                *output_gain_db,
            )))
        }
        AudioFilterConfig::NoiseGate {
            open_threshold_db,
            close_threshold_db,
            attack_ms,
            hold_ms,
            release_ms,
        } => {
            Some(Box::new(noise_gate::NoiseGateFilter::new(
                *open_threshold_db,
                *close_threshold_db,
                *attack_ms,
                *hold_ms,
                *release_ms,
            )))
        }
        AudioFilterConfig::Expander {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
        } => {
            Some(Box::new(expander::ExpanderFilter::new(
                *threshold_db,
                *ratio,
                *attack_ms,
                *release_ms,
            )))
        }
        AudioFilterConfig::Limiter {
            threshold_db,
            release_ms,
        } => {
            Some(Box::new(limiter::LimiterFilter::new(*threshold_db, *release_ms)))
        }
        AudioFilterConfig::NoiseSuppression { suppress_level } => {
            Some(Box::new(noise_suppression::NoiseSuppressionFilter::new(*suppress_level)))
        }
    }
}
