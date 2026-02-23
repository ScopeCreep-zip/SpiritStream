// Sync Delay Buffer
// Implements per-source audio sync offset (positive = delay audio)
// Uses a VecDeque as a FIFO delay line

use std::borrow::Cow;
use std::collections::VecDeque;

/// Per-source sync delay buffer.
/// Delays audio by a configurable number of milliseconds.
pub struct SyncDelayBuffer {
    buffer: VecDeque<f32>,
    delay_samples: usize,
}

impl SyncDelayBuffer {
    /// Create a new delay buffer.
    ///
    /// # Arguments
    /// * `delay_ms` - Delay in milliseconds (positive only, negative is handled at mixer level)
    /// * `sample_rate` - Sample rate in Hz
    /// * `channels` - Number of channels
    pub fn new(delay_ms: i64, sample_rate: u32, channels: usize) -> Self {
        let delay_samples = Self::ms_to_samples(delay_ms, sample_rate, channels);
        let mut buffer = VecDeque::with_capacity(delay_samples + 1024);
        // Pre-fill with silence for the initial delay
        buffer.extend(std::iter::repeat(0.0f32).take(delay_samples));
        Self {
            buffer,
            delay_samples,
        }
    }

    /// Process a chunk of interleaved samples through the delay line.
    /// Returns the delayed samples. Output length equals input length.
    /// Zero-delay passthrough borrows the input (zero-copy).
    pub fn process<'a>(&mut self, input: &'a [f32]) -> Cow<'a, [f32]> {
        if self.delay_samples == 0 {
            return Cow::Borrowed(input);
        }

        // Push input samples into buffer
        self.buffer.extend(input.iter());

        // Drain delayed samples from front
        let output_len = input.len();
        let mut output = Vec::with_capacity(output_len);
        for _ in 0..output_len {
            output.push(self.buffer.pop_front().unwrap_or(0.0));
        }
        Cow::Owned(output)
    }

    /// Update the delay. Resizes the internal buffer.
    pub fn set_delay(&mut self, delay_ms: i64, sample_rate: u32, channels: usize) {
        let new_delay = Self::ms_to_samples(delay_ms, sample_rate, channels);
        if new_delay == self.delay_samples {
            return;
        }

        if new_delay > self.delay_samples {
            // Increasing delay: insert silence at front
            let extra = new_delay - self.delay_samples;
            for _ in 0..extra {
                self.buffer.push_front(0.0);
            }
        } else {
            // Decreasing delay: remove samples from front
            let remove = self.delay_samples - new_delay;
            for _ in 0..remove {
                self.buffer.pop_front();
            }
        }
        self.delay_samples = new_delay;
    }

    /// Current delay in samples
    pub fn delay_samples(&self) -> usize {
        self.delay_samples
    }

    pub fn ms_to_samples(delay_ms: i64, sample_rate: u32, channels: usize) -> usize {
        if delay_ms <= 0 {
            return 0;
        }
        (delay_ms as usize * sample_rate as usize * channels / 1000).max(0)
    }
}
