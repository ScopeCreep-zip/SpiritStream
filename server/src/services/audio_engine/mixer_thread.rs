// Audio Mixer Thread
// Dedicated OS thread running the mix loop at ~21.3ms intervals (1024 samples @ 48kHz).
// Named "ss-audio-mixer", runs at ThreadPriority::Max.
//
// Loop per tick:
// 1. For each registered source: read from rtrb::Consumer<f32>
// 2. Resample if native rate != 48kHz
// 3. Apply sync delay buffer
// 4. (Phase 2: Apply FilterChain.process())
// 5. Multiply by volume * (muted ? 0.0 : 1.0) with constant-power balance
// 6. Accumulate into track buffers based on track_bitmask
// 7. Write mixed track buffers to outputs
// 8. Tap: call audio_level_service.update_source_level() per source
// 9. Tap: call audio_level_service.update_track_level() per track

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use parking_lot::Mutex;

use crate::models::{FaderCurve, SourceAudioConfig};
use crate::services::AudioLevelService;
use crate::services::audio_filters::FilterChain;
use super::monitor_output::{should_monitor, should_output};
use super::resampler::SourceResampler;
use super::sync_delay::SyncDelayBuffer;
use super::types::{MixerConfig, MAX_AUDIO_TRACKS};

/// Apply a fader curve to a linear volume value.
///
/// OBS supports three curves:
/// - Cubic (default): volume^3 — perceptually uniform, emphasizes quiet end
/// - Linear: volume — direct 1:1 mapping
/// - Sine: sin(volume * PI/2) — gentle taper at extremes
///
/// Input `volume` is in the range 0.0..=20.0 (OBS range), output is the
/// gain multiplier to apply to audio samples.
fn apply_fader_curve(volume: f32, curve: &FaderCurve) -> f32 {
    // Normalize to 0.0..=1.0 for curve application, then scale back
    // OBS fader range is 0..1 internally; the 0..20x is the gain after curve.
    // We apply the curve to the normalized position, then multiply by max gain.
    //
    // For simplicity and OBS parity: treat volume as the fader position (0..1 = 0%..100%)
    // when volume <= 1.0, and as raw gain when > 1.0 (boost range).
    if volume <= 0.0 {
        return 0.0;
    }
    if volume > 1.0 {
        // Boost range (1.0..20.0) — applied linearly regardless of curve
        // The curve only shapes the 0..1 fader range
        return volume;
    }
    match curve {
        FaderCurve::Cubic => volume * volume * volume,
        FaderCurve::Linear => volume,
        FaderCurve::Sine => (volume * std::f32::consts::FRAC_PI_2).sin(),
    }
}

/// Per-source state maintained by the mixer thread
struct SourceMixState {
    consumer: rtrb::Consumer<f32>,
    resampler: SourceResampler,
    sync_delay: SyncDelayBuffer,
    filter_chain: FilterChain,
    /// Config version last seen — detects any audio config change (not just filter count)
    last_config_version: u64,
}

/// Start the mixer thread. Returns the join handle.
///
/// If `monitor_producer` is `Some`, sources with `MonitorOnly` or `MonitorAndOutput`
/// monitoring types will have their post-filter audio written to the monitor output.
/// Sources with `MonitorOnly` are excluded from the normal track routing.
pub fn start_mixer_thread(
    config: MixerConfig,
    source_configs: Arc<DashMap<String, Arc<SourceAudioConfig>>>,
    source_consumers: Arc<Mutex<HashMap<String, (rtrb::Consumer<f32>, u32, u16)>>>,
    stop_flag: Arc<AtomicBool>,
    audio_level_service: Arc<AudioLevelService>,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("ss-audio-mixer".into())
        .spawn(move || {
            // Set Mach real-time scheduling (macOS) or max QoS priority (other platforms)
            #[cfg(not(target_os = "ios"))]
            {
                use crate::services::thread_config;
                thread_config::set_realtime_audio_priority(
                    config.samples_per_callback as u32,
                    config.sample_rate,
                );
            }

            log::info!(
                "[AudioEngine] Mixer thread started ({}Hz, {} channels, {} samples/tick)",
                config.sample_rate,
                config.channels(),
                config.samples_per_callback
            );

            let tick_duration = Duration::from_secs_f64(
                config.samples_per_callback as f64 / config.sample_rate as f64,
            );
            let channels = config.channels();
            let samples_per_tick = config.samples_per_callback * channels;

            // Per-source state (built lazily as consumers appear)
            let mut source_states: HashMap<String, SourceMixState> = HashMap::new();

            // Reusable buffers
            let mut read_buf = vec![0.0f32; samples_per_tick * 2]; // oversized for safety
            let mut track_bufs: Vec<Vec<f32>> = (0..MAX_AUDIO_TRACKS)
                .map(|_| vec![0.0f32; samples_per_tick])
                .collect();
            let mut master_buf = vec![0.0f32; samples_per_tick];
            let mut monitor_buf = vec![0.0f32; samples_per_tick];

            // Pre-allocate track key strings (avoids format!() allocation per tick)
            let track_keys: Vec<String> = (0..MAX_AUDIO_TRACKS)
                .map(|i| format!("__track_{}", i + 1))
                .collect();

            while !stop_flag.load(Ordering::Relaxed) {
                let tick_start = Instant::now();

                // Drain new consumers from the registration queue
                {
                    let mut pending = source_consumers.lock();
                    for (source_id, (consumer, sample_rate, source_channels)) in pending.drain() {
                        let resampler = SourceResampler::new(
                            sample_rate,
                            config.sample_rate,
                            source_channels as usize,
                            config.samples_per_callback,
                        );

                        // Get sync offset from config (Arc clone is cheap — atomic bump)
                        let delay_ms = source_configs
                            .get(&source_id)
                            .map(|c| c.sync_offset_ms)
                            .unwrap_or(0);

                        let sync_delay = SyncDelayBuffer::new(
                            delay_ms,
                            config.sample_rate,
                            channels,
                        );

                        // Build initial filter chain from config (Arc deref)
                        let (filter_chain, config_version) = source_configs
                            .get(&source_id)
                            .map(|entry| (FilterChain::from_configs(&entry.audio_filters), entry.config_version))
                            .unwrap_or_else(|| (FilterChain::new(), 0));

                        source_states.insert(source_id.clone(), SourceMixState {
                            consumer,
                            resampler,
                            sync_delay,
                            filter_chain,
                            last_config_version: config_version,
                        });

                        log::info!(
                            "[AudioEngine] Source '{}' connected to mixer ({}Hz, {}ch, resampling={})",
                            source_id, sample_rate, source_channels,
                            sample_rate != config.sample_rate
                        );
                    }
                }

                // Clear track buffers
                for buf in &mut track_bufs {
                    buf.iter_mut().for_each(|s| *s = 0.0);
                }
                master_buf.iter_mut().for_each(|s| *s = 0.0);
                monitor_buf.iter_mut().for_each(|s| *s = 0.0);

                // Remove disconnected sources (producer dropped)
                source_states.retain(|id, state| {
                    if state.consumer.is_abandoned() {
                        log::info!("[AudioEngine] Source '{}' disconnected (producer dropped)", id);
                        // Reset mixer_active so raw broadcast metering can resume
                        audio_level_service.reset_mixer_active(id);
                        false
                    } else {
                        true
                    }
                });

                // Check if any source has solo enabled
                let any_solo = source_configs.iter().any(|entry| entry.value().solo);

                // Mix each source
                for (source_id, state) in &mut source_states {
                    // Read available samples from ring buffer (drain-to-latest pattern)
                    let available = state.consumer.slots();
                    if available == 0 {
                        continue;
                    }

                    // Read up to samples_per_tick (or whatever is available)
                    let to_read = available.min(samples_per_tick);
                    let chunk = match state.consumer.read_chunk(to_read) {
                        Ok(chunk) => chunk,
                        Err(_) => continue,
                    };

                    let slice = chunk.as_slices();
                    // Copy into contiguous buffer
                    let total = slice.0.len() + slice.1.len();
                    if total > read_buf.len() {
                        read_buf.resize(total, 0.0);
                    }
                    read_buf[..slice.0.len()].copy_from_slice(slice.0);
                    read_buf[slice.0.len()..total].copy_from_slice(slice.1);
                    chunk.commit_all();

                    let samples = &read_buf[..total];

                    // Get source config (Arc clone = atomic bump, not struct clone)
                    let audio_config = source_configs
                        .get(source_id)
                        .map(|entry| Arc::clone(&*entry))
                        .unwrap_or_else(|| Arc::new(SourceAudioConfig::default()));

                    // Update sync delay if config changed at runtime
                    let target_delay_samples = SyncDelayBuffer::ms_to_samples(
                        audio_config.sync_offset_ms,
                        config.sample_rate,
                        channels,
                    );
                    if state.sync_delay.delay_samples() != target_delay_samples {
                        state.sync_delay.set_delay(
                            audio_config.sync_offset_ms,
                            config.sample_rate,
                            channels,
                        );
                        log::debug!(
                            "[AudioEngine] Source '{}' sync delay updated to {}ms ({} samples)",
                            source_id, audio_config.sync_offset_ms, target_delay_samples
                        );
                    }

                    // Step 2: Resample if needed (Cow::Borrowed on passthrough)
                    let resampled = state.resampler.process(samples);

                    // Step 3: Apply sync delay (Cow::Borrowed on zero delay)
                    let delayed = state.sync_delay.process(&resampled);

                    // Step 4: Apply filter chain (gain, compressor, gate, etc.)
                    // Rebuild chain if config_version changed (detects parameter changes, not just count)
                    if audio_config.config_version != state.last_config_version {
                        state.filter_chain = FilterChain::from_configs(&audio_config.audio_filters);
                        state.last_config_version = audio_config.config_version;
                    }
                    // Only materialize owned Vec when filter chain needs to mutate
                    let delayed = if !state.filter_chain.is_empty() {
                        let mut owned = delayed.into_owned();
                        state.filter_chain.process(&mut owned, channels, config.sample_rate);
                        Cow::Owned(owned)
                    } else {
                        delayed
                    };

                    // Compute pre-fader metering (input_peak for gain staging display)
                    let (pre_rms_l, pre_rms_r, input_peak_l, input_peak_r) =
                        compute_stereo_levels(&delayed, channels);

                    // Step 4.5: Monitor output — write pre-mix audio for monitored sources
                    let monitoring_type = audio_config.monitoring_type;
                    if should_monitor(&monitoring_type) {
                        // Accumulate into monitor buffer (pre-volume, post-filter)
                        for (i, &sample) in delayed.iter().enumerate() {
                            if i < monitor_buf.len() {
                                monitor_buf[i] += sample;
                            }
                        }
                    }

                    // Step 5: Apply volume, mute, solo, balance, fader curve
                    let muted = audio_config.muted || (any_solo && !audio_config.solo);
                    let raw_gain = if muted {
                        0.0
                    } else {
                        apply_fader_curve(audio_config.volume, &audio_config.fader_curve)
                    };
                    let gain = raw_gain;
                    let balance = audio_config.balance.clamp(-1.0, 1.0);

                    // Constant-power balance (equal-power panning)
                    let angle = (balance + 1.0) * std::f32::consts::FRAC_PI_4; // 0 to PI/2
                    let gain_l = gain * angle.cos();
                    let gain_r = gain * angle.sin();

                    // Step 6: Accumulate into track buffers based on bitmask
                    // MonitorOnly sources are excluded from track routing.
                    if should_output(&monitoring_type) {
                        let bitmask = audio_config.track_bitmask;
                        for track_idx in 0..MAX_AUDIO_TRACKS {
                            if bitmask & (1 << track_idx) == 0 {
                                continue;
                            }

                            let track_buf = &mut track_bufs[track_idx];
                            for (i, sample) in delayed.iter().enumerate() {
                                if i >= track_buf.len() {
                                    break;
                                }
                                // Apply L/R gain based on channel position
                                let ch_gain = if channels >= 2 {
                                    if i % channels == 0 { gain_l } else { gain_r }
                                } else {
                                    gain // mono
                                };
                                track_buf[i] += sample * ch_gain;
                            }
                        }
                    }

                    // Step 7: Post-fader metering — shows what the audience hears.
                    // Mathematical multiplication is exact because gain is constant across the buffer.
                    let post_rms_l = pre_rms_l * gain_l.abs();
                    let post_rms_r = pre_rms_r * gain_r.abs();
                    let post_peak_l = if muted { 0.0 } else { input_peak_l * gain_l.abs() };
                    let post_peak_r = if muted { 0.0 } else { input_peak_r * gain_r.abs() };

                    audio_level_service.update_source_level_from_mixer(
                        source_id,
                        post_rms_l, post_rms_r,
                        post_peak_l, post_peak_r,
                        input_peak_l, input_peak_r,
                    );
                }

                // Step 8: Compute master sum and per-track metering
                for (track_idx, track_buf) in track_bufs.iter().enumerate() {
                    // Accumulate into master
                    for (i, &sample) in track_buf.iter().enumerate() {
                        if i < master_buf.len() {
                            master_buf[i] += sample;
                        }
                    }

                    // Per-track metering (emitted as __track_1 through __track_6)
                    let (t_rms_l, t_rms_r, t_peak_l, t_peak_r) =
                        compute_stereo_levels(track_buf, channels);

                    // Only emit if there's actual signal
                    if t_peak_l > 0.0001 || t_peak_r > 0.0001 {
                        audio_level_service.update_source_level(
                            &track_keys[track_idx], t_rms_l, t_rms_r, t_peak_l, t_peak_r,
                        );
                    }
                }

                // Sleep until next tick
                let elapsed = tick_start.elapsed();
                if elapsed < tick_duration {
                    std::thread::sleep(tick_duration - elapsed);
                }
            }

            log::info!("[AudioEngine] Mixer thread stopped");
        })
        .expect("Failed to spawn audio mixer thread")
}

/// Compute stereo RMS and peak from interleaved samples.
/// Returns (rms_l, rms_r, peak_l, peak_r).
fn compute_stereo_levels(samples: &[f32], channels: usize) -> (f32, f32, f32, f32) {
    if samples.is_empty() || channels == 0 {
        return (0.0, 0.0, 0.0, 0.0);
    }

    if channels >= 2 {
        let mut sum_sq_l = 0.0f32;
        let mut sum_sq_r = 0.0f32;
        let mut max_l = 0.0f32;
        let mut max_r = 0.0f32;
        let mut count = 0usize;

        for frame in samples.chunks(channels) {
            if frame.len() >= 2 {
                let l = frame[0];
                let r = frame[1];
                sum_sq_l += l * l;
                sum_sq_r += r * r;
                max_l = max_l.max(l.abs());
                max_r = max_r.max(r.abs());
                count += 1;
            }
        }

        if count > 0 {
            (
                (sum_sq_l / count as f32).sqrt(),
                (sum_sq_r / count as f32).sqrt(),
                max_l,
                max_r,
            )
        } else {
            (0.0, 0.0, 0.0, 0.0)
        }
    } else {
        // Mono
        let mut sum_sq = 0.0f32;
        let mut max_abs = 0.0f32;
        for &s in samples {
            sum_sq += s * s;
            max_abs = max_abs.max(s.abs());
        }
        let rms = (sum_sq / samples.len() as f32).sqrt();
        (rms, rms, max_abs, max_abs)
    }
}
