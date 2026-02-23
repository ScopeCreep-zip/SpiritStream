// Expander Filter
// Downward expansion below threshold — reduces gain for quiet signals.
// Inverse of compressor: expands the dynamic range below the threshold.
// OBS reference: plugins/obs-filters/expander-filter.c

use super::AudioFilter;

pub struct ExpanderFilter {
    threshold_db: f32,
    ratio: f32,
    attack_coeff: f32,
    release_coeff: f32,
    envelope_db: f32,
    cached_sample_rate: u32,
    attack_ms: f32,
    release_ms: f32,
}

impl ExpanderFilter {
    pub fn new(threshold_db: f32, ratio: f32, attack_ms: f32, release_ms: f32) -> Self {
        Self {
            threshold_db,
            ratio: ratio.max(1.0),
            attack_coeff: 0.0,
            release_coeff: 0.0,
            envelope_db: -96.0,
            cached_sample_rate: 0,
            attack_ms,
            release_ms,
        }
    }

    fn update_coefficients(&mut self, sample_rate: u32) {
        if sample_rate == self.cached_sample_rate {
            return;
        }
        self.cached_sample_rate = sample_rate;
        let sr = sample_rate as f32;
        self.attack_coeff = (-1.0 / (self.attack_ms * 0.001 * sr)).exp();
        self.release_coeff = (-1.0 / (self.release_ms * 0.001 * sr)).exp();
    }
}

impl AudioFilter for ExpanderFilter {
    fn process(&mut self, samples: &mut [f32], channels: usize, sample_rate: u32) {
        self.update_coefficients(sample_rate);
        let ch = channels.max(1);

        for frame in samples.chunks_mut(ch) {
            let peak = frame.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
            let input_db = linear_to_db(peak);

            // Smooth envelope
            let coeff = if input_db > self.envelope_db {
                self.attack_coeff
            } else {
                self.release_coeff
            };
            self.envelope_db = coeff * self.envelope_db + (1.0 - coeff) * input_db;

            // Downward expansion: reduce gain when envelope is BELOW threshold
            let gain_db = if self.envelope_db < self.threshold_db {
                let under = self.threshold_db - self.envelope_db;
                // Expand: multiply the "under" by (ratio - 1) to push it further down
                -(under * (self.ratio - 1.0))
            } else {
                0.0
            };

            let gain = db_to_linear(gain_db);
            for s in frame.iter_mut() {
                *s *= gain;
            }
        }
    }

    fn reset(&mut self) {
        self.envelope_db = -96.0;
    }

    fn filter_type(&self) -> &str {
        "expander"
    }
}

fn db_to_linear(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}

fn linear_to_db(linear: f32) -> f32 {
    if linear > 1e-6 {
        20.0 * linear.log10()
    } else {
        -96.0
    }
}
