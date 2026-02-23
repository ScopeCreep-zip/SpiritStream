// Gain Filter
// Simple linear gain multiply. Converts dB to linear on construction.

use super::AudioFilter;

pub struct GainFilter {
    gain_linear: f32,
}

impl GainFilter {
    pub fn new(gain_db: f32) -> Self {
        Self {
            gain_linear: db_to_linear(gain_db),
        }
    }
}

impl AudioFilter for GainFilter {
    fn process(&mut self, samples: &mut [f32], _channels: usize, _sample_rate: u32) {
        if (self.gain_linear - 1.0).abs() < f32::EPSILON {
            return; // Unity gain — skip
        }
        for s in samples.iter_mut() {
            *s *= self.gain_linear;
        }
    }

    fn reset(&mut self) {}

    fn filter_type(&self) -> &str {
        "gain"
    }
}

fn db_to_linear(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}
