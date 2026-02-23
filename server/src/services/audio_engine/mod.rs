// Audio Engine Service
// Central audio mixing engine with per-source processing pipeline.
//
// Architecture (OBS parity):
// - One mixer thread at ThreadPriority::Max (~21.3ms tick at 1024 samples / 48kHz)
// - Per-source: ring buffer input → resampler → sync delay → filter chain → volume/balance → track routing
// - 6 output tracks, plus master sum
// - Capture callbacks write to rtrb::Producer (wait-free, no allocation)
// - Mixer thread reads from rtrb::Consumer, processes, and taps metering

pub mod types;
pub mod resampler;
pub mod sync_delay;
pub mod mixer_thread;
pub mod monitor_output;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use dashmap::DashMap;
use parking_lot::Mutex;
use tokio::sync::broadcast;

use crate::models::SourceAudioConfig;
use crate::services::audio_capture::AudioBuffer;
use crate::services::AudioLevelService;
use types::{MixerConfig, RING_BUFFER_CAPACITY};

/// Central audio mixing engine service.
///
/// Manages per-source ring buffers, runtime audio configs, and the mixer thread.
/// Integrates with AudioCaptureService (producers) and AudioLevelService (metering taps).
///
/// `source_configs` uses `Arc<SourceAudioConfig>` so the mixer thread can clone the Arc
/// (atomic ref count bump) instead of cloning the entire struct each tick.
pub struct AudioEngineService {
    config: MixerConfig,
    /// Per-source runtime configs (volume, mute, etc.)
    source_configs: Arc<DashMap<String, Arc<SourceAudioConfig>>>,
    /// Mixer thread handle
    mixer_handle: std::sync::Mutex<Option<std::thread::JoinHandle<()>>>,
    /// Stop signal for mixer thread
    stop_flag: Arc<AtomicBool>,
    /// Pending source consumers to be picked up by mixer thread.
    /// Key: source_id, Value: (Consumer, sample_rate, channels)
    pending_consumers: Arc<Mutex<HashMap<String, (rtrb::Consumer<f32>, u32, u16)>>>,
    /// Active producer handles (kept alive while source is registered)
    active_producers: Mutex<HashMap<String, ()>>,
}

impl AudioEngineService {
    pub fn new(config: MixerConfig) -> Self {
        Self {
            config,
            source_configs: Arc::new(DashMap::new()),
            mixer_handle: std::sync::Mutex::new(None),
            stop_flag: Arc::new(AtomicBool::new(false)),
            pending_consumers: Arc::new(Mutex::new(HashMap::new())),
            active_producers: Mutex::new(HashMap::new()),
        }
    }

    /// Register a source for mixing. Returns the rtrb::Producer that
    /// the capture callback should write samples into.
    ///
    /// The producer is wait-free — safe to call from audio callbacks.
    pub fn register_source(
        &self,
        source_id: &str,
        sample_rate: u32,
        channels: u16,
    ) -> rtrb::Producer<f32> {
        // Create ring buffer
        let (producer, consumer) = rtrb::RingBuffer::new(RING_BUFFER_CAPACITY);

        // Ensure default audio config exists
        self.source_configs
            .entry(source_id.to_string())
            .or_insert_with(|| Arc::new(SourceAudioConfig::default()));

        // Queue consumer for mixer thread pickup
        self.pending_consumers
            .lock()
            .insert(source_id.to_string(), (consumer, sample_rate, channels));

        // Track active producer
        self.active_producers
            .lock()
            .insert(source_id.to_string(), ());

        log::info!(
            "[AudioEngine] Registered source '{}' ({}Hz, {}ch, ring_buf={})",
            source_id, sample_rate, channels, RING_BUFFER_CAPACITY
        );

        producer
    }

    /// Register a source from a broadcast receiver (capture pipeline bridge).
    ///
    /// Bridges a `broadcast::Receiver<AudioBuffer>` to the internal `rtrb::Producer<f32>`
    /// interface. This is the primary way capture services feed audio into the mixer.
    ///
    /// Returns a `JoinHandle` that can be used for lifecycle tracking — when the
    /// broadcast sender is dropped, the task will exit cleanly.
    pub fn register_source_from_broadcast(
        &self,
        source_id: &str,
        sample_rate: u32,
        channels: u16,
        mut rx: broadcast::Receiver<AudioBuffer>,
    ) -> tokio::task::JoinHandle<()> {
        let mut producer = self.register_source(source_id, sample_rate, channels);
        let source_id_owned = source_id.to_string();

        tokio::spawn(async move {
            log::info!(
                "[AudioEngine] Broadcast bridge started for source '{}'",
                source_id_owned
            );
            loop {
                match rx.recv().await {
                    Ok(buffer) => {
                        // Write samples into the ring buffer for the mixer thread.
                        // If the ring buffer is full (mixer thread hasn't drained yet),
                        // we silently drop — this is the correct backpressure behavior
                        // for real-time audio.
                        if let Ok(mut chunk) =
                            producer.write_chunk_uninit(buffer.samples.len())
                        {
                            let (first, second) = chunk.as_mut_slices();
                            let samples = &buffer.samples;
                            let first_len = first.len();

                            for (slot, &sample) in first.iter_mut().zip(&samples[..first_len]) {
                                slot.write(sample);
                            }
                            if !second.is_empty() {
                                for (slot, &sample) in
                                    second.iter_mut().zip(&samples[first_len..])
                                {
                                    slot.write(sample);
                                }
                            }
                            unsafe {
                                chunk.commit_all();
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        log::debug!(
                            "[AudioEngine] Source '{}' broadcast lagged {} buffers",
                            source_id_owned,
                            n
                        );
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        log::info!(
                            "[AudioEngine] Source '{}' broadcast closed, stopping bridge",
                            source_id_owned
                        );
                        break;
                    }
                }
            }
        })
    }

    /// Unregister a source. Stops mixing it.
    /// The consumer will be detected as abandoned by the mixer thread.
    pub fn unregister_source(&self, source_id: &str) {
        self.active_producers.lock().remove(source_id);
        self.pending_consumers.lock().remove(source_id);
        // Note: source_configs are NOT removed here — they persist for profile save.
        // The mixer thread will detect the abandoned consumer and clean up.
        log::info!("[AudioEngine] Unregistered source '{}'", source_id);
    }

    /// Update runtime audio config for a source (called from mixer commands).
    pub fn update_source_config(&self, source_id: &str, config: SourceAudioConfig) {
        self.source_configs.insert(source_id.to_string(), Arc::new(config));
    }

    /// Get current config for a source
    pub fn get_source_config(&self, source_id: &str) -> Option<SourceAudioConfig> {
        self.source_configs.get(source_id).map(|c| (**c).clone())
    }

    /// Load all configs from a profile (called on profile load)
    pub fn load_configs(&self, configs: &HashMap<String, SourceAudioConfig>) {
        for (id, config) in configs {
            self.source_configs.insert(id.clone(), Arc::new(config.clone()));
        }
    }

    /// Start the mixer thread.
    pub fn start(&self, audio_level_service: Arc<AudioLevelService>) {
        if self.stop_flag.swap(false, Ordering::SeqCst) {
            log::warn!("[AudioEngine] Mixer was previously stopped, resetting stop flag");
        }

        let handle = mixer_thread::start_mixer_thread(
            self.config.clone(),
            self.source_configs.clone(),
            self.pending_consumers.clone(),
            self.stop_flag.clone(),
            audio_level_service,
        );

        if let Ok(mut h) = self.mixer_handle.lock() {
            *h = Some(handle);
        }

        log::info!("[AudioEngine] Mixer thread started");
    }

    /// Stop the mixer thread.
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);

        if let Ok(mut h) = self.mixer_handle.lock() {
            if let Some(handle) = h.take() {
                let _ = handle.join();
            }
        }

        log::info!("[AudioEngine] Mixer thread stopped");
    }

    /// Check if the mixer thread is running
    pub fn is_running(&self) -> bool {
        !self.stop_flag.load(Ordering::Relaxed)
    }
}

impl Drop for AudioEngineService {
    fn drop(&mut self) {
        self.stop();
    }
}
