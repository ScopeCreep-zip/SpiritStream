// Audio Levels Service
// Monitors audio sources and emits real level data to WebSocket clients
// Unified audio bus: all source types register via broadcast::Receiver<AudioBuffer>

use crate::services::audio_capture::AudioBuffer;
use crate::services::events::{emit_event, EventSink};
use bytes::{BufMut, BytesMut};
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};

/// Binary audio frame magic byte and version
const BINARY_MAGIC: u8 = 0xAF;
const BINARY_VERSION: u8 = 1;

/// Audio level data for a single track
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioLevel {
    /// RMS level (0.0 - 1.0)
    pub rms: f32,
    /// Peak level (0.0 - 1.0)
    pub peak: f32,
    /// Whether clipping was detected
    pub clipping: bool,
    /// Left channel RMS (0.0 - 1.0) for stereo sources
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left_rms: Option<f32>,
    /// Left channel peak (0.0 - 1.0) for stereo sources
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left_peak: Option<f32>,
    /// Right channel RMS (0.0 - 1.0) for stereo sources
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right_rms: Option<f32>,
    /// Right channel peak (0.0 - 1.0) for stereo sources
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right_peak: Option<f32>,
    /// Peak level in dB for display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_db: Option<f32>,
    /// Pre-fader peak L (for gain staging display). Only present when mixer provides it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_peak_l: Option<f32>,
    /// Pre-fader peak R (for gain staging display). Only present when mixer provides it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_peak_r: Option<f32>,
}

impl Default for AudioLevel {
    fn default() -> Self {
        Self {
            rms: 0.0,
            peak: 0.0,
            clipping: false,
            left_rms: None,
            left_peak: None,
            right_rms: None,
            right_peak: None,
            peak_db: None,
            input_peak_l: None,
            input_peak_r: None,
        }
    }
}

/// Audio levels data sent via WebSocket
#[derive(Debug, Clone, Serialize)]
pub struct AudioLevelsData {
    /// Per-track audio levels keyed by source ID
    pub tracks: HashMap<String, AudioLevel>,
    /// Master output level
    pub master: AudioLevel,
}

/// OBS metering constants
/// RMS integration window: 3 samples at 10Hz = ~300ms (OBS uses ~300ms)
const RMS_WINDOW_SIZE: usize = 3;
/// Peak hold duration: 20 seconds (OBS parity)
const PEAK_HOLD_SECS: f64 = 20.0;
/// OBS PPM peak decay rate: 6.92 dB/s (11.76 dB over 1.7 seconds)
const PEAK_DECAY_DB_PER_SEC: f64 = 6.92;

/// Internal state for smoothing and peak hold
struct TrackState {
    /// Peak hold for L channel
    peak_hold_l: f32,
    /// Peak hold for R channel
    peak_hold_r: f32,
    /// Time when peak hold started for L channel
    peak_hold_start_l: Instant,
    /// Time when peak hold started for R channel
    peak_hold_start_r: Instant,
    /// Recent RMS samples for smoothing (L channel)
    /// Using VecDeque for O(1) pop_front instead of Vec::remove(0) which is O(n)
    rms_history_l: VecDeque<f32>,
    /// Recent RMS samples for smoothing (R channel)
    rms_history_r: VecDeque<f32>,
    /// Running sum for L channel RMS (O(1) average computation)
    rms_sum_l: f32,
    /// Running sum for R channel RMS (O(1) average computation)
    rms_sum_r: f32,
}

impl Default for TrackState {
    fn default() -> Self {
        Self {
            peak_hold_l: 0.0,
            peak_hold_r: 0.0,
            peak_hold_start_l: Instant::now(),
            peak_hold_start_r: Instant::now(),
            rms_history_l: VecDeque::with_capacity(RMS_WINDOW_SIZE + 1),
            rms_history_r: VecDeque::with_capacity(RMS_WINDOW_SIZE + 1),
            rms_sum_l: 0.0,
            rms_sum_r: 0.0,
        }
    }
}

/// Tracked source data including level and last update time
/// Follows OBS's audio metering model with separate RMS and Peak per channel
#[derive(Debug, Clone)]
struct TrackedSource {
    /// Left channel RMS level (0.0 - 1.0) - average power (post-fader)
    rms_l: f32,
    /// Right channel RMS level (0.0 - 1.0) - average power (post-fader)
    rms_r: f32,
    /// Left channel peak level (0.0 - 1.0) - instantaneous max (post-fader)
    peak_l: f32,
    /// Right channel peak level (0.0 - 1.0) - instantaneous max (post-fader)
    peak_r: f32,
    /// Pre-fader peak L (for gain staging display)
    input_peak_l: Option<f32>,
    /// Pre-fader peak R (for gain staging display)
    input_peak_r: Option<f32>,
    /// Last time this source received an update
    last_update: Instant,
    /// Whether the mixer thread is actively writing to this source.
    /// When true, the raw broadcast metering task yields (mixer has priority).
    mixer_active: bool,
}

impl Default for TrackedSource {
    fn default() -> Self {
        Self {
            rms_l: 0.0,
            rms_r: 0.0,
            peak_l: 0.0,
            peak_r: 0.0,
            input_peak_l: None,
            input_peak_r: None,
            last_update: Instant::now(),
            mixer_active: false,
        }
    }
}

/// Combined audio state — single mutex for both tracked sources and smoothing state.
/// Eliminates double-lock on 10Hz monitoring tick.
struct AudioState {
    tracked: HashMap<String, TrackedSource>,
    smoothing: HashMap<String, TrackState>,
}

/// Audio level monitoring service - real levels only, no simulation
///
/// Unified audio bus: all source types (cpal, SCK, symphonia, rsmpeg) register
/// via `register_audio_source()` which accepts a `broadcast::Receiver<AudioBuffer>`.
/// A background task per source computes RMS/peak and calls `update_source_level()`.
/// Mute is handled by the AudioEngineService mixer thread (gain=0 when muted),
/// so metering naturally reports zeros for muted sources.
pub struct AudioLevelService {
    /// Running state
    running: Arc<AtomicBool>,
    /// Combined tracked sources and smoothing state (single lock)
    state: Arc<Mutex<AudioState>>,
    /// Handle to the monitoring task for clean shutdown
    task_handle: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Background tasks spawned by register_audio_source (one per source)
    source_tasks: Mutex<HashMap<String, tokio::task::JoinHandle<()>>>,
}

impl AudioLevelService {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            state: Arc::new(Mutex::new(AudioState {
                tracked: HashMap::new(),
                smoothing: HashMap::new(),
            })),
            task_handle: std::sync::Mutex::new(None),
            source_tasks: Mutex::new(HashMap::new()),
        }
    }

    /// Check if monitoring is active
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Set the list of tracked source IDs
    pub fn set_tracked_sources(&self, source_ids: Vec<String>) {
        let mut state = self.state.lock();

        // Remove sources no longer tracked
        state.tracked.retain(|id, _| source_ids.contains(id));
        state.smoothing.retain(|id, _| source_ids.contains(id));

        // Add new sources with zero level
        for id in source_ids {
            state.tracked.entry(id.clone()).or_insert_with(TrackedSource::default);
            state.smoothing.entry(id).or_insert_with(TrackState::default);
        }
    }

    /// Update the level for a specific source (called from raw broadcast capture task).
    /// If the mixer thread is actively writing to this source (`mixer_active`), this
    /// method returns early — the mixer's post-fader levels take priority.
    ///
    /// Follows OBS's audio metering model:
    /// - RMS = root mean square (average power)
    /// - Peak = instantaneous maximum absolute sample value
    /// For mono sources, pass the same value for both channels.
    pub fn update_source_level(
        &self,
        source_id: &str,
        rms_l: f32,
        rms_r: f32,
        peak_l: f32,
        peak_r: f32,
    ) {
        let mut state = self.state.lock();
        if let Some(tracked) = state.tracked.get_mut(source_id) {
            // Yield to mixer thread when it's active (post-fader levels are authoritative)
            if tracked.mixer_active {
                return;
            }
            tracked.rms_l = rms_l.clamp(0.0, 1.0);
            tracked.rms_r = rms_r.clamp(0.0, 1.0);
            tracked.peak_l = peak_l.clamp(0.0, 1.0);
            tracked.peak_r = peak_r.clamp(0.0, 1.0);
            tracked.last_update = Instant::now();
        } else {
            // Log once per source to avoid spam
            static WARNED: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> = std::sync::OnceLock::new();
            let warned = WARNED.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()));
            if let Ok(mut set) = warned.lock() {
                if set.insert(source_id.to_string()) {
                    log::warn!("Audio level update for untracked source '{}'. Tracked sources: {:?}",
                        source_id, state.tracked.keys().collect::<Vec<_>>());
                }
            }
        }
    }

    /// Update the level for a specific source from the mixer thread (post-fader).
    /// Sets `mixer_active = true` so the raw broadcast task yields.
    /// Includes pre-fader `input_peak` for gain staging display.
    pub fn update_source_level_from_mixer(
        &self,
        source_id: &str,
        rms_l: f32,
        rms_r: f32,
        peak_l: f32,
        peak_r: f32,
        input_peak_l: f32,
        input_peak_r: f32,
    ) {
        let mut state = self.state.lock();
        if let Some(tracked) = state.tracked.get_mut(source_id) {
            tracked.mixer_active = true;
            tracked.rms_l = rms_l.clamp(0.0, 1.0);
            tracked.rms_r = rms_r.clamp(0.0, 1.0);
            tracked.peak_l = peak_l.clamp(0.0, 1.0);
            tracked.peak_r = peak_r.clamp(0.0, 1.0);
            tracked.input_peak_l = Some(input_peak_l.clamp(0.0, 1.0));
            tracked.input_peak_r = Some(input_peak_r.clamp(0.0, 1.0));
            tracked.last_update = Instant::now();
        }
    }

    /// Reset `mixer_active` for a source so the raw broadcast task can resume.
    /// Called when the mixer drops a source (producer abandoned).
    pub fn reset_mixer_active(&self, source_id: &str) {
        let mut state = self.state.lock();
        if let Some(tracked) = state.tracked.get_mut(source_id) {
            tracked.mixer_active = false;
            tracked.input_peak_l = None;
            tracked.input_peak_r = None;
        }
    }

    /// Get health status for all tracked sources
    /// A source is considered "healthy" if it received an update within the last 2 seconds
    pub fn get_health_status(&self) -> HashMap<String, bool> {
        let state = self.state.lock();
        let timeout = Duration::from_secs(2);

        state.tracked
            .iter()
            .map(|(id, tracked)| {
                let healthy = tracked.last_update.elapsed() < timeout;
                (id.clone(), healthy)
            })
            .collect()
    }

    // ========================================================================
    // Unified Audio Bus: register/unregister/mute
    // ========================================================================

    /// Register an audio source for unified metering.
    ///
    /// Spawns a background tokio task that receives `AudioBuffer` from the
    /// broadcast channel, computes stereo RMS/peak, checks mute state,
    /// and calls `update_source_level()`.
    ///
    /// Returns a `JoinHandle` that can be used for lifecycle tracking.
    pub fn register_audio_source(
        self: &Arc<Self>,
        source_id: &str,
        mut rx: broadcast::Receiver<AudioBuffer>,
    ) {
        let source_id_owned = source_id.to_string();
        let this = Arc::clone(self);

        let handle = tokio::spawn(async move {
            log::info!("[AudioBus] Registered source '{}' for unified metering", source_id_owned);
            let mut buffer_count: u64 = 0;
            loop {
                match rx.recv().await {
                    Ok(buffer) => {
                        buffer_count += 1;

                        // Mute is handled by the mixer thread (gain=0),
                        // so metering naturally shows zeros for muted sources.
                        let (rms_l, rms_r, peak_l, peak_r) = compute_stereo_levels(&buffer);

                        this.update_source_level(&source_id_owned, rms_l, rms_r, peak_l, peak_r);

                        if buffer_count <= 3 || buffer_count % 500 == 0 {
                            log::debug!(
                                "[AudioBus] Source '{}' buffer #{}: RMS L={:.4} R={:.4}, Peak L={:.4} R={:.4}",
                                source_id_owned, buffer_count, rms_l, rms_r, peak_l, peak_r
                            );
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        log::debug!("[AudioBus] Source '{}' lagged {} buffers", source_id_owned, n);
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        log::info!("[AudioBus] Source '{}' channel closed, stopping metering", source_id_owned);
                        break;
                    }
                }
            }
        });

        self.source_tasks.lock().insert(source_id.to_string(), handle);
    }

    /// Unregister an audio source — aborts its background metering task.
    pub fn unregister_audio_source(&self, source_id: &str) {
        if let Some(handle) = self.source_tasks.lock().remove(source_id) {
            handle.abort();
            log::info!("[AudioBus] Unregistered source '{}'", source_id);
        }
    }

    /// Unregister all sources that are NOT in the provided set of active source IDs.
    pub fn unregister_removed_sources(&self, active_ids: &[String]) {
        let mut tasks = self.source_tasks.lock();
        let stale: Vec<String> = tasks.keys()
            .filter(|id| !active_ids.contains(id))
            .cloned()
            .collect();
        for id in stale {
            if let Some(handle) = tasks.remove(&id) {
                handle.abort();
                log::info!("[AudioBus] Unregistered removed source '{}'", id);
            }
        }
    }

    /// Start the monitoring loop
    /// When `idle_flag` is set to true, reduces update rate from 10Hz to 2Hz
    /// to save CPU/battery when the app UI is not visible.
    /// When `throttle_flag` is set (thermal pressure), also uses the idle interval.
    pub fn start<E: EventSink + 'static>(
        &self,
        event_sink: Arc<E>,
        idle_flag: Arc<AtomicBool>,
        throttle_flag: Arc<AtomicBool>,
    ) {
        if self.running.swap(true, Ordering::Relaxed) {
            log::debug!("AudioLevelService already running");
            return;
        }

        let running = self.running.clone();
        let audio_state = self.state.clone();

        let handle = tokio::spawn(async move {
            log::info!("AudioLevelService started (real levels only)");

            // Normal: 10Hz (100ms) — sufficient for visual metering
            // Idle: 2Hz (500ms) — minimal updates when tab is hidden
            let normal_interval = Duration::from_millis(100);
            let idle_interval = Duration::from_millis(500);
            let mut ticker = interval(normal_interval);
            let mut emit_count: u64 = 0;
            let mut was_idle = false;

            while running.load(Ordering::Relaxed) {
                ticker.tick().await;

                // Adjust tick rate when idle or thermally throttled
                let is_idle = idle_flag.load(Ordering::Relaxed)
                    || throttle_flag.load(Ordering::Relaxed);
                if is_idle != was_idle {
                    was_idle = is_idle;
                    let new_interval = if is_idle { idle_interval } else { normal_interval };
                    ticker = interval(new_interval);
                    ticker.tick().await; // First tick completes immediately
                    log::debug!("AudioLevelService tick rate changed to {}ms (idle={}, throttle={})",
                        new_interval.as_millis(),
                        idle_flag.load(Ordering::Relaxed),
                        throttle_flag.load(Ordering::Relaxed));
                }

                let mut state = audio_state.lock();

                if state.tracked.is_empty() {
                    // No sources to monitor — skip all work (near-zero CPU when idle)
                    drop(state);
                    continue;
                }

                let mut tracks = HashMap::new();
                let mut master_rms_sum = 0.0;
                let mut master_rms_l_sum = 0.0;
                let mut master_rms_r_sum = 0.0;
                let mut master_peak = 0.0f32;
                let mut master_peak_l = 0.0f32;
                let mut master_peak_r = 0.0f32;
                let mut master_clipping = false;

                // Collect source IDs to iterate (needed because we mutate smoothing in the same state)
                let source_ids: Vec<String> = state.tracked.keys().cloned().collect();

                for source_id in &source_ids {
                    let tracked = state.tracked.get(source_id).unwrap().clone();
                    let sm = state.smoothing.entry(source_id.clone()).or_insert_with(TrackState::default);

                    // RMS values (average power) - apply smoothing with running sums (O(1) per tick)
                    // Window size: RMS_WINDOW_SIZE samples at 10Hz = ~300ms (OBS parity)
                    // Smooth L channel RMS
                    sm.rms_sum_l += tracked.rms_l;
                    sm.rms_history_l.push_back(tracked.rms_l);
                    if sm.rms_history_l.len() > RMS_WINDOW_SIZE {
                        sm.rms_sum_l -= sm.rms_history_l.pop_front().unwrap();
                    }
                    let smoothed_rms_l = sm.rms_sum_l / sm.rms_history_l.len() as f32;

                    // Smooth R channel RMS
                    sm.rms_sum_r += tracked.rms_r;
                    sm.rms_history_r.push_back(tracked.rms_r);
                    if sm.rms_history_r.len() > RMS_WINDOW_SIZE {
                        sm.rms_sum_r -= sm.rms_history_r.pop_front().unwrap();
                    }
                    let smoothed_rms_r = sm.rms_sum_r / sm.rms_history_r.len() as f32;

                    // Combined RMS for overall level
                    let smoothed_rms = ((smoothed_rms_l.powi(2) + smoothed_rms_r.powi(2)) / 2.0).sqrt();

                    // Peak values — OBS-style peak hold state machine:
                    // 1. If new peak is higher, update and restart hold timer
                    // 2. During hold period (20s), keep the peak value
                    // 3. After hold expires, linear decay over 3 seconds to zero
                    let now_instant = Instant::now();

                    // Peak hold for L channel
                    if tracked.peak_l > sm.peak_hold_l {
                        sm.peak_hold_l = tracked.peak_l;
                        sm.peak_hold_start_l = now_instant;
                    } else {
                        let hold_elapsed = now_instant.duration_since(sm.peak_hold_start_l).as_secs_f64();
                        if hold_elapsed > PEAK_HOLD_SECS {
                            // OBS PPM dB-linear decay: 6.92 dB/s
                            let decay_elapsed = hold_elapsed - PEAK_HOLD_SECS;
                            let decay_db = PEAK_DECAY_DB_PER_SEC * decay_elapsed;
                            let decay_factor = 10.0_f64.powf(-decay_db / 20.0) as f32;
                            sm.peak_hold_l *= decay_factor;
                            if sm.peak_hold_l < 0.001 {
                                sm.peak_hold_l = 0.0;
                            }
                        }
                        // During hold period: peak_hold_l stays unchanged
                    }

                    // Peak hold for R channel
                    if tracked.peak_r > sm.peak_hold_r {
                        sm.peak_hold_r = tracked.peak_r;
                        sm.peak_hold_start_r = now_instant;
                    } else {
                        let hold_elapsed = now_instant.duration_since(sm.peak_hold_start_r).as_secs_f64();
                        if hold_elapsed > PEAK_HOLD_SECS {
                            // OBS PPM dB-linear decay: 6.92 dB/s
                            let decay_elapsed = hold_elapsed - PEAK_HOLD_SECS;
                            let decay_db = PEAK_DECAY_DB_PER_SEC * decay_elapsed;
                            let decay_factor = 10.0_f64.powf(-decay_db / 20.0) as f32;
                            sm.peak_hold_r *= decay_factor;
                            if sm.peak_hold_r < 0.001 {
                                sm.peak_hold_r = 0.0;
                            }
                        }
                        // During hold period: peak_hold_r stays unchanged
                    }

                    // Overall peak (max of L and R)
                    let overall_peak = sm.peak_hold_l.max(sm.peak_hold_r);
                    let clipping = overall_peak > 0.95;

                    // Calculate dB for display
                    // Clamp peak to 1.0 to prevent positive dB values (which indicate clipping)
                    // Values above 1.0 are still represented as 0 dB with clipping flag set
                    let clamped_peak = overall_peak.min(1.0);
                    let peak_db = if clamped_peak > 0.0 {
                        Some(20.0 * clamped_peak.log10())
                    } else {
                        Some(-96.0)
                    };

                    tracks.insert(source_id.clone(), AudioLevel {
                        rms: smoothed_rms,
                        peak: overall_peak,
                        clipping,
                        left_rms: Some(smoothed_rms_l),
                        left_peak: Some(sm.peak_hold_l),
                        right_rms: Some(smoothed_rms_r),
                        right_peak: Some(sm.peak_hold_r),
                        peak_db,
                        input_peak_l: tracked.input_peak_l,
                        input_peak_r: tracked.input_peak_r,
                    });

                    // Accumulate for master
                    master_rms_sum += smoothed_rms * smoothed_rms;
                    master_rms_l_sum += smoothed_rms_l * smoothed_rms_l;
                    master_rms_r_sum += smoothed_rms_r * smoothed_rms_r;
                    master_peak = master_peak.max(overall_peak);
                    master_peak_l = master_peak_l.max(sm.peak_hold_l);
                    master_peak_r = master_peak_r.max(sm.peak_hold_r);
                    if clipping {
                        master_clipping = true;
                    }
                }

                // Calculate master RMS
                let num_sources = state.tracked.len() as f32;
                let master_rms = (master_rms_sum / num_sources).sqrt();
                let master_rms_l = (master_rms_l_sum / num_sources).sqrt();
                let master_rms_r = (master_rms_r_sum / num_sources).sqrt();

                // Calculate master dB
                // Clamp peak to 1.0 to prevent positive dB values
                let clamped_master_peak = master_peak.min(1.0);
                let master_peak_db = if clamped_master_peak > 0.0 {
                    Some(20.0 * clamped_master_peak.log10())
                } else {
                    Some(-96.0)
                };

                let data = AudioLevelsData {
                    tracks,
                    master: AudioLevel {
                        rms: master_rms,
                        peak: master_peak,
                        clipping: master_clipping,
                        left_rms: Some(master_rms_l),
                        left_peak: Some(master_peak_l),
                        right_rms: Some(master_rms_r),
                        right_peak: Some(master_peak_r),
                        peak_db: master_peak_db,
                        input_peak_l: None,
                        input_peak_r: None,
                    },
                };

                emit_count += 1;
                // Log first 5 emits, then every 100th (about every 3 seconds)
                if emit_count <= 5 || emit_count % 100 == 0 {
                    log::info!("[AudioLevelService] Emit #{}: {} tracks, master_rms={:.4}, track_levels={:?}",
                        emit_count,
                        data.tracks.len(),
                        master_rms,
                        data.tracks.iter().map(|(id, l)| (id.clone(), l.rms)).collect::<Vec<_>>()
                    );
                }

                // Emit binary frame for high-performance WebSocket transport
                let binary = encode_levels_binary(&data);
                event_sink.emit_binary("audio_levels", binary);

                // Also emit JSON for backward compatibility (other event consumers)
                emit_event(event_sink.as_ref(), "audio_levels", &data);
            }

            log::info!("AudioLevelService stopped");
        });

        // Store handle for clean shutdown
        if let Ok(mut h) = self.task_handle.lock() {
            *h = Some(handle);
        }
    }

    /// Stop the monitoring loop and all registered source tasks.
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        if let Ok(mut h) = self.task_handle.lock() {
            if let Some(handle) = h.take() {
                handle.abort();
            }
        }
        // Abort all registered source metering tasks
        let mut tasks = self.source_tasks.lock();
        for (id, handle) in tasks.drain() {
            handle.abort();
            log::debug!("[AudioBus] Aborted metering task for source '{}'", id);
        }
    }
}

impl Default for AudioLevelService {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AudioLevelService {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Encode audio levels into compact binary format for WebSocket transport.
///
/// Wire format (little-endian):
///   [1 byte magic 0xAF] [1 byte version] [2 bytes track count (u16le)]
///   [MASTER_BLOCK: 9 × f32le + 1 byte flags = 37 bytes]
///   [TRACK_BLOCK × N: 1 byte id_len, id_len bytes UTF-8 id, 9 × f32le + 1 byte flags]
///
/// f32 fields: rmsL, rmsR, peakL, peakR, peakDb, inputPeakL, inputPeakR, rms, peak
/// flags: bit 0 = clipping, bit 1 = has inputPeak
fn encode_levels_binary(data: &AudioLevelsData) -> bytes::Bytes {
    // Pre-allocate: header(4) + master(37) + tracks(~50 bytes each)
    let estimated = 4 + 37 + data.tracks.len() * 80;
    let mut buf = BytesMut::with_capacity(estimated);

    // Header
    buf.put_u8(BINARY_MAGIC);
    buf.put_u8(BINARY_VERSION);
    buf.put_u16_le(data.tracks.len() as u16);

    // Write a level block (9 × f32le + 1 byte flags)
    fn write_block(buf: &mut BytesMut, level: &AudioLevel) {
        buf.put_f32_le(level.left_rms.unwrap_or(level.rms));
        buf.put_f32_le(level.right_rms.unwrap_or(level.rms));
        buf.put_f32_le(level.left_peak.unwrap_or(level.peak));
        buf.put_f32_le(level.right_peak.unwrap_or(level.peak));
        buf.put_f32_le(level.peak_db.unwrap_or(-96.0));
        buf.put_f32_le(level.input_peak_l.unwrap_or(0.0));
        buf.put_f32_le(level.input_peak_r.unwrap_or(0.0));
        buf.put_f32_le(level.rms);
        buf.put_f32_le(level.peak);

        let mut flags: u8 = 0;
        if level.clipping {
            flags |= 0x01;
        }
        if level.input_peak_l.is_some() {
            flags |= 0x02;
        }
        buf.put_u8(flags);
    }

    // Master block
    write_block(&mut buf, &data.master);

    // Track blocks
    for (id, level) in &data.tracks {
        let id_bytes = id.as_bytes();
        buf.put_u8(id_bytes.len().min(255) as u8);
        buf.put_slice(&id_bytes[..id_bytes.len().min(255)]);
        write_block(&mut buf, level);
    }

    buf.freeze()
}

/// Compute stereo RMS and peak from an `AudioBuffer`.
///
/// Returns `(rms_l, rms_r, peak_l, peak_r)`.
/// - Mono (1 channel): same value for L and R.
/// - Stereo (2+ channels): uses channels 0 (L) and 1 (R).
/// - Empty buffer: all zeros.
pub fn compute_stereo_levels(buffer: &AudioBuffer) -> (f32, f32, f32, f32) {
    let channels = buffer.channels as usize;
    if buffer.samples.is_empty() || channels == 0 {
        return (0.0, 0.0, 0.0, 0.0);
    }

    if channels >= 2 {
        let mut sum_sq_l = 0.0f32;
        let mut sum_sq_r = 0.0f32;
        let mut max_abs_l = 0.0f32;
        let mut max_abs_r = 0.0f32;
        let mut count = 0usize;
        for frame in buffer.samples.chunks_exact(channels) {
            let sample_l = frame[0];
            let sample_r = frame[1];
            sum_sq_l += sample_l * sample_l;
            sum_sq_r += sample_r * sample_r;
            max_abs_l = max_abs_l.max(sample_l.abs());
            max_abs_r = max_abs_r.max(sample_r.abs());
            count += 1;
        }
        let rms_l = if count > 0 { (sum_sq_l / count as f32).sqrt() } else { 0.0 };
        let rms_r = if count > 0 { (sum_sq_r / count as f32).sqrt() } else { 0.0 };
        (rms_l, rms_r, max_abs_l, max_abs_r)
    } else {
        // Mono
        let mut sum_sq = 0.0f32;
        let mut max_abs = 0.0f32;
        for &sample in &buffer.samples {
            sum_sq += sample * sample;
            max_abs = max_abs.max(sample.abs());
        }
        let rms = (sum_sq / buffer.samples.len() as f32).sqrt();
        (rms, rms, max_abs, max_abs)
    }
}
