// Audio Resampler
// Converts audio from source sample rate to mixer output rate using rubato FFT resampler

use std::borrow::Cow;
use rubato::{FftFixedInOut, Resampler};

/// Per-source resampler. Passthrough when input rate matches output rate.
pub struct SourceResampler {
    resampler: Option<FftFixedInOut<f32>>,
    channels: usize,
    /// Intermediate buffers for deinterleaving (reused across calls)
    input_bufs: Vec<Vec<f32>>,
}

impl SourceResampler {
    /// Create a new resampler. Returns passthrough if rates match.
    ///
    /// # Arguments
    /// * `input_rate` - Source native sample rate
    /// * `output_rate` - Mixer output sample rate (typically 48000)
    /// * `channels` - Number of audio channels
    /// * `chunk_size` - Samples per channel per chunk
    pub fn new(input_rate: u32, output_rate: u32, channels: usize, chunk_size: usize) -> Self {
        let resampler = if input_rate != output_rate {
            match FftFixedInOut::<f32>::new(
                input_rate as usize,
                output_rate as usize,
                chunk_size,
                channels,
            ) {
                Ok(r) => Some(r),
                Err(e) => {
                    log::error!(
                        "Failed to create resampler ({}Hz → {}Hz): {}. Using passthrough.",
                        input_rate, output_rate, e
                    );
                    None
                }
            }
        } else {
            None
        };

        let input_bufs = vec![Vec::with_capacity(chunk_size); channels];

        Self {
            resampler,
            channels,
            input_bufs,
        }
    }

    /// Process interleaved samples. Returns resampled interleaved output.
    /// If no resampling needed, borrows the input (zero-copy passthrough).
    pub fn process<'a>(&mut self, input: &'a [f32]) -> Cow<'a, [f32]> {
        match &mut self.resampler {
            Some(resampler) => {
                let frames = input.len() / self.channels;

                // Deinterleave into per-channel buffers
                for buf in &mut self.input_bufs {
                    buf.clear();
                }
                // Ensure we have enough channel buffers
                while self.input_bufs.len() < self.channels {
                    self.input_bufs.push(Vec::with_capacity(frames));
                }

                for frame_idx in 0..frames {
                    for ch in 0..self.channels {
                        let sample_idx = frame_idx * self.channels + ch;
                        if sample_idx < input.len() {
                            self.input_bufs[ch].push(input[sample_idx]);
                        }
                    }
                }

                // Pad to required input length if needed
                let required_len = resampler.input_frames_next();
                for buf in &mut self.input_bufs {
                    buf.resize(required_len, 0.0);
                }

                // Process through resampler — stack-allocate refs for stereo (common case)
                let refs: Cow<[&[f32]]> = if self.channels == 2 {
                    Cow::Borrowed(&[
                        self.input_bufs[0].as_slice(),
                        self.input_bufs[1].as_slice(),
                    ] as &[&[f32]])
                } else {
                    Cow::Owned(self.input_bufs.iter().map(|b| b.as_slice()).collect())
                };
                match resampler.process(&refs, None) {
                    Ok(output_bufs) => {
                        // Reinterleave
                        let out_frames = output_bufs.first().map(|b| b.len()).unwrap_or(0);
                        let mut interleaved = Vec::with_capacity(out_frames * self.channels);
                        for frame_idx in 0..out_frames {
                            for ch in 0..self.channels {
                                interleaved.push(
                                    output_bufs.get(ch)
                                        .and_then(|b| b.get(frame_idx))
                                        .copied()
                                        .unwrap_or(0.0)
                                );
                            }
                        }
                        Cow::Owned(interleaved)
                    }
                    Err(e) => {
                        log::warn!("Resampler error: {}. Passing through.", e);
                        Cow::Borrowed(input)
                    }
                }
            }
            // Passthrough — zero-copy borrow
            None => Cow::Borrowed(input),
        }
    }

    /// Whether this resampler actually resamples (vs passthrough)
    pub fn is_active(&self) -> bool {
        self.resampler.is_some()
    }
}
