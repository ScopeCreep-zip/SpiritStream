// Audio Levels Service
// Monitors audio sources and emits real level data to WebSocket clients

use crate::services::events::{emit_event, EventSink};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};

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

/// Internal state for smoothing and peak hold
struct TrackState {
    /// Peak hold for L channel
    peak_hold_l: f32,
    /// Peak hold for R channel
    peak_hold_r: f32,
    /// Recent RMS samples for smoothing (L channel)
    /// Using VecDeque for O(1) pop_front instead of Vec::remove(0) which is O(n)
    rms_history_l: VecDeque<f32>,
    /// Recent RMS samples for smoothing (R channel)
    rms_history_r: VecDeque<f32>,
}

impl Default for TrackState {
    fn default() -> Self {
        Self {
            peak_hold_l: 0.0,
            peak_hold_r: 0.0,
            rms_history_l: VecDeque::with_capacity(8),
            rms_history_r: VecDeque::with_capacity(8),
        }
    }
}

/// Tracked source data including level and last update time
/// Follows OBS's audio metering model with separate RMS and Peak per channel
#[derive(Debug, Clone)]
struct TrackedSource {
    /// Left channel RMS level (0.0 - 1.0) - average power
    rms_l: f32,
    /// Right channel RMS level (0.0 - 1.0) - average power
    rms_r: f32,
    /// Left channel peak level (0.0 - 1.0) - instantaneous max
    peak_l: f32,
    /// Right channel peak level (0.0 - 1.0) - instantaneous max
    peak_r: f32,
    /// Last time this source received an update
    last_update: Instant,
}

impl Default for TrackedSource {
    fn default() -> Self {
        Self {
            rms_l: 0.0,
            rms_r: 0.0,
            peak_l: 0.0,
            peak_r: 0.0,
            last_update: Instant::now(),
        }
    }
}

/// Audio level monitoring service - real levels only, no simulation
pub struct AudioLevelService {
    /// Running state
    running: Arc<AtomicBool>,
    /// Tracked source IDs and their data (level + last update time)
    tracked_sources: Arc<Mutex<HashMap<String, TrackedSource>>>,
    /// Internal track states for smoothing
    track_states: Arc<Mutex<HashMap<String, TrackState>>>,
}

impl AudioLevelService {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            tracked_sources: Arc::new(Mutex::new(HashMap::new())),
            track_states: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if monitoring is active
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Set the list of tracked source IDs
    pub async fn set_tracked_sources(&self, source_ids: Vec<String>) {
        let mut sources = self.tracked_sources.lock().await;
        let mut states = self.track_states.lock().await;

        // Remove sources no longer tracked
        sources.retain(|id, _| source_ids.contains(id));
        states.retain(|id, _| source_ids.contains(id));

        // Add new sources with zero level
        for id in source_ids {
            sources.entry(id.clone()).or_insert_with(TrackedSource::default);
            states.entry(id).or_insert_with(TrackState::default);
        }
    }

    /// Update the level for a specific source (called from real audio capture)
    /// Follows OBS's audio metering model:
    /// - RMS = root mean square (average power)
    /// - Peak = instantaneous maximum absolute sample value
    /// For mono sources, pass the same value for both channels
    pub async fn update_source_level(
        &self,
        source_id: &str,
        rms_l: f32,
        rms_r: f32,
        peak_l: f32,
        peak_r: f32,
    ) {
        let mut sources = self.tracked_sources.lock().await;
        if let Some(tracked) = sources.get_mut(source_id) {
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
                        source_id, sources.keys().collect::<Vec<_>>());
                }
            }
        }
    }

    /// Get health status for all tracked sources
    /// A source is considered "healthy" if it received an update within the last 2 seconds
    pub async fn get_health_status(&self) -> HashMap<String, bool> {
        let sources = self.tracked_sources.lock().await;
        let timeout = Duration::from_secs(2);

        sources
            .iter()
            .map(|(id, tracked)| {
                let healthy = tracked.last_update.elapsed() < timeout;
                (id.clone(), healthy)
            })
            .collect()
    }

    /// Start the monitoring loop
    pub fn start<E: EventSink + 'static>(&self, event_sink: Arc<E>) {
        if self.running.swap(true, Ordering::Relaxed) {
            log::debug!("AudioLevelService already running");
            return;
        }

        let running = self.running.clone();
        let tracked_sources = self.tracked_sources.clone();
        let track_states = self.track_states.clone();

        tokio::spawn(async move {
            log::info!("AudioLevelService started (real levels only)");

            // 20Hz update rate (OBS parity) - 50ms intervals
            // Previously 30Hz (33ms), reduced to match OBS and decrease WebSocket traffic
            let mut ticker = interval(Duration::from_millis(50));
            let mut emit_count: u64 = 0;

            while running.load(Ordering::Relaxed) {
                ticker.tick().await;

                let sources = tracked_sources.lock().await;
                let mut states = track_states.lock().await;

                if sources.is_empty() {
                    // No sources to monitor, emit empty data
                    let data = AudioLevelsData {
                        tracks: HashMap::new(),
                        master: AudioLevel::default(),
                    };
                    emit_event(event_sink.as_ref(), "audio_levels", &data);
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

                for (source_id, tracked) in sources.iter() {
                    let state = states.entry(source_id.clone()).or_insert_with(TrackState::default);

                    // RMS values (average power) - apply smoothing
                    // Smooth L channel RMS using O(1) VecDeque operations
                    state.rms_history_l.push_back(tracked.rms_l);
                    if state.rms_history_l.len() > 6 {
                        state.rms_history_l.pop_front();
                    }
                    let smoothed_rms_l = state.rms_history_l.iter().sum::<f32>() / state.rms_history_l.len() as f32;

                    // Smooth R channel RMS using O(1) VecDeque operations
                    state.rms_history_r.push_back(tracked.rms_r);
                    if state.rms_history_r.len() > 6 {
                        state.rms_history_r.pop_front();
                    }
                    let smoothed_rms_r = state.rms_history_r.iter().sum::<f32>() / state.rms_history_r.len() as f32;

                    // Combined RMS for overall level
                    let smoothed_rms = ((smoothed_rms_l.powi(2) + smoothed_rms_r.powi(2)) / 2.0).sqrt();

                    // Peak values (instantaneous max) - apply peak hold with decay
                    // OBS-style peak hold: hold at max, then decay after hold time
                    // Peak hold for L channel (from actual peak, not RMS)
                    if tracked.peak_l > state.peak_hold_l {
                        state.peak_hold_l = tracked.peak_l;
                    } else {
                        // Decay coefficient ~0.98 at 30Hz = ~660ms decay to 50%
                        state.peak_hold_l = (state.peak_hold_l * 0.98).max(tracked.peak_l * 0.5);
                    }

                    // Peak hold for R channel (from actual peak, not RMS)
                    if tracked.peak_r > state.peak_hold_r {
                        state.peak_hold_r = tracked.peak_r;
                    } else {
                        state.peak_hold_r = (state.peak_hold_r * 0.98).max(tracked.peak_r * 0.5);
                    }

                    // Overall peak (max of L and R)
                    let overall_peak = state.peak_hold_l.max(state.peak_hold_r);
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
                        left_peak: Some(state.peak_hold_l),
                        right_rms: Some(smoothed_rms_r),
                        right_peak: Some(state.peak_hold_r),
                        peak_db,
                    });

                    // Accumulate for master
                    master_rms_sum += smoothed_rms * smoothed_rms;
                    master_rms_l_sum += smoothed_rms_l * smoothed_rms_l;
                    master_rms_r_sum += smoothed_rms_r * smoothed_rms_r;
                    master_peak = master_peak.max(overall_peak);
                    master_peak_l = master_peak_l.max(state.peak_hold_l);
                    master_peak_r = master_peak_r.max(state.peak_hold_r);
                    if clipping {
                        master_clipping = true;
                    }
                }

                // Calculate master RMS
                let num_sources = sources.len() as f32;
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

                emit_event(event_sink.as_ref(), "audio_levels", &data);
            }

            log::info!("AudioLevelService stopped");
        });
    }

    /// Stop the monitoring loop
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
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
