// Noise Suppression Filter
// RNNoise-based background noise removal via the nnnoiseless crate.
// Handles format bridging: mixer's stereo interleaved f32 [-1,1]
// → nnnoiseless's mono 480-sample i16-scale f32 frames.
// Stereo preserved: one DenoiseState per channel (OBS pattern from noise-suppress-filter.c).

use std::collections::VecDeque;

use super::AudioFilter;
use nnnoiseless::DenoiseState;

const FRAME_SIZE: usize = DenoiseState::FRAME_SIZE; // 480
const SCALE_UP: f32 = 32767.0;
const SCALE_DOWN: f32 = 1.0 / 32767.0;

pub struct NoiseSuppressionFilter {
    /// One DenoiseState per channel (RNNoise is mono-only).
    states: Vec<Box<DenoiseState>>,
    /// Per-channel accumulation buffer (fills until 480 samples ready).
    input_bufs: Vec<VecDeque<f32>>,
    /// Per-channel denoised output waiting to be drained.
    /// Uses VecDeque for O(1) pop_front instead of Vec::remove(0) which is O(n).
    output_bufs: Vec<VecDeque<f32>>,
    /// Dry samples saved before processing for wet/dry blend.
    dry_bufs: Vec<VecDeque<f32>>,
    /// Wet/dry mix: 0.0 = bypass, 1.0 = full suppression.
    mix: f32,
    /// Channel count from last process() call (lazy init on change).
    cached_channels: usize,
}

impl NoiseSuppressionFilter {
    pub fn new(suppress_level: f32) -> Self {
        Self {
            states: Vec::new(),
            input_bufs: Vec::new(),
            output_bufs: Vec::new(),
            dry_bufs: Vec::new(),
            mix: suppress_level.clamp(0.0, 1.0),
            cached_channels: 0,
        }
    }

    fn init_channels(&mut self, channels: usize) {
        self.cached_channels = channels;
        self.states.clear();
        self.input_bufs.clear();
        self.output_bufs.clear();
        self.dry_bufs.clear();
        for _ in 0..channels {
            self.states.push(DenoiseState::new());
            self.input_bufs.push(VecDeque::with_capacity(FRAME_SIZE * 2));
            self.output_bufs.push(VecDeque::with_capacity(FRAME_SIZE * 2));
            self.dry_bufs.push(VecDeque::with_capacity(FRAME_SIZE * 2));
        }
    }
}

impl AudioFilter for NoiseSuppressionFilter {
    fn process(&mut self, samples: &mut [f32], channels: usize, _sample_rate: u32) {
        let ch = channels.max(1);

        if self.mix < 1e-6 {
            return;
        }

        if ch != self.cached_channels {
            self.init_channels(ch);
        }

        // 1. Deinterleave into per-channel buffers, save dry copy
        for frame in samples.chunks(ch) {
            for (c, &s) in frame.iter().enumerate() {
                self.input_bufs[c].push_back(s);
                self.dry_bufs[c].push_back(s);
            }
        }

        // 2. Process complete 480-sample frames through nnnoiseless
        let mut frame_in = [0.0f32; FRAME_SIZE];
        let mut frame_out = [0.0f32; FRAME_SIZE];

        for c in 0..ch {
            while self.input_bufs[c].len() >= FRAME_SIZE {
                // Drain 480 samples from front (O(1) per pop_front)
                for slot in &mut frame_in {
                    *slot = self.input_bufs[c].pop_front().unwrap_or(0.0) * SCALE_UP;
                }

                self.states[c].process_frame(&mut frame_out, &frame_in);

                for &s in &frame_out {
                    self.output_bufs[c].push_back(s * SCALE_DOWN);
                }
            }
        }

        // 3. Write back interleaved with wet/dry blend
        let num_frames = samples.len() / ch;
        let wet = self.mix;
        let dry = 1.0 - wet;

        for i in 0..num_frames {
            for c in 0..ch {
                let idx = i * ch + c;
                if idx >= samples.len() {
                    break;
                }

                if !self.output_bufs[c].is_empty() {
                    let processed = self.output_bufs[c].pop_front().unwrap_or(0.0);
                    let original = if !self.dry_bufs[c].is_empty() {
                        self.dry_bufs[c].pop_front().unwrap_or(samples[idx])
                    } else {
                        samples[idx]
                    };
                    samples[idx] = wet * processed + dry * original;
                } else if !self.dry_bufs[c].is_empty() {
                    // Not enough processed yet — output dry (latency warmup)
                    samples[idx] = self.dry_bufs[c].pop_front().unwrap_or(samples[idx]);
                }
            }
        }
    }

    fn reset(&mut self) {
        let ch = self.cached_channels;
        if ch > 0 {
            self.init_channels(ch);
        }
    }

    fn filter_type(&self) -> &str {
        "noise_suppression"
    }
}
