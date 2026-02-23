// Compressor Filter
// Envelope-follower compressor with configurable threshold, ratio, attack, release,
// and output gain. Processes per-sample with smoothed envelope detection.
//
// OBS reference: plugins/obs-filters/compressor-filter.c

use super::AudioFilter;

pub struct CompressorFilter {
    threshold_db: f32,
    ratio: f32,
    attack_coeff: f32,
    release_coeff: f32,
    output_gain_linear: f32,
    /// Current envelope level in dB (tracks signal loudness)
    envelope_db: f32,
    /// Cached sample rate for coefficient recalculation
    cached_sample_rate: u32,
    /// Raw parameters for recalculation
    attack_ms: f32,
    release_ms: f32,
}

impl CompressorFilter {
    pub fn new(
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        output_gain_db: f32,
    ) -> Self {
        // Coefficients will be computed on first process() call when sample_rate is known
        Self {
            threshold_db,
            ratio: ratio.max(1.0),
            attack_coeff: 0.0,
            release_coeff: 0.0,
            output_gain_linear: db_to_linear(output_gain_db),
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

impl AudioFilter for CompressorFilter {
    fn process(&mut self, samples: &mut [f32], channels: usize, sample_rate: u32) {
        self.update_coefficients(sample_rate);

        for frame in samples.chunks_mut(channels.max(1)) {
            // Compute peak of this frame
            let peak = frame.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
            let input_db = linear_to_db(peak);

            // Smooth envelope (attack/release)
            let coeff = if input_db > self.envelope_db {
                self.attack_coeff
            } else {
                self.release_coeff
            };
            self.envelope_db = coeff * self.envelope_db + (1.0 - coeff) * input_db;

            // Compute gain reduction
            let gain_db = if self.envelope_db > self.threshold_db {
                let over = self.envelope_db - self.threshold_db;
                // Reduce the "over" portion by (1 - 1/ratio)
                -(over * (1.0 - 1.0 / self.ratio))
            } else {
                0.0
            };

            let gain = db_to_linear(gain_db) * self.output_gain_linear;

            for s in frame.iter_mut() {
                *s *= gain;
            }
        }
    }

    fn reset(&mut self) {
        self.envelope_db = -96.0;
    }

    fn filter_type(&self) -> &str {
        "compressor"
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
