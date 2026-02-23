// Limiter Filter
// Brick-wall limiter — instant attack, configurable release.
// Prevents signal from exceeding the threshold.
// OBS reference: plugins/obs-filters/limiter-filter.c

use super::AudioFilter;

pub struct LimiterFilter {
    threshold_linear: f32,
    release_coeff: f32,
    /// Current gain reduction (1.0 = no reduction)
    current_gain: f32,
    cached_sample_rate: u32,
    release_ms: f32,
}

impl LimiterFilter {
    pub fn new(threshold_db: f32, release_ms: f32) -> Self {
        Self {
            threshold_linear: db_to_linear(threshold_db),
            release_coeff: 0.0,
            current_gain: 1.0,
            cached_sample_rate: 0,
            release_ms,
        }
    }

    fn update_coefficients(&mut self, sample_rate: u32) {
        if sample_rate == self.cached_sample_rate {
            return;
        }
        self.cached_sample_rate = sample_rate;
        let sr = sample_rate as f32;
        self.release_coeff = (-1.0 / (self.release_ms * 0.001 * sr)).exp();
    }
}

impl AudioFilter for LimiterFilter {
    fn process(&mut self, samples: &mut [f32], channels: usize, sample_rate: u32) {
        self.update_coefficients(sample_rate);
        let ch = channels.max(1);

        for frame in samples.chunks_mut(ch) {
            let peak = frame.iter().map(|s| s.abs()).fold(0.0f32, f32::max);

            // Instant attack: if peak exceeds threshold, compute required gain
            let target_gain = if peak > self.threshold_linear {
                self.threshold_linear / peak
            } else {
                1.0
            };

            // Apply: instant attack (min), smooth release
            if target_gain < self.current_gain {
                // Instant attack
                self.current_gain = target_gain;
            } else {
                // Smooth release
                self.current_gain =
                    self.release_coeff * self.current_gain + (1.0 - self.release_coeff) * target_gain;
            }

            for s in frame.iter_mut() {
                *s *= self.current_gain;
            }
        }
    }

    fn reset(&mut self) {
        self.current_gain = 1.0;
    }

    fn filter_type(&self) -> &str {
        "limiter"
    }
}

fn db_to_linear(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}
