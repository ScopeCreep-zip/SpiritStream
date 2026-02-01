// Audio Levels Service
// Monitors audio sources and emits level data to WebSocket clients

use crate::services::events::{emit_event, EventSink};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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

/// Internal state for calculating audio levels
struct TrackState {
    /// Recent peak samples for decay (L channel)
    peak_hold_l: f32,
    /// Recent peak samples for decay (R channel)
    peak_hold_r: f32,
    /// Recent RMS samples for smoothing (L channel)
    rms_history_l: Vec<f32>,
    /// Recent RMS samples for smoothing (R channel)
    rms_history_r: Vec<f32>,
    /// Simulation phase for L channel
    sim_phase_l: f32,
    /// Simulation phase for R channel
    sim_phase_r: f32,
    /// Simulation frequency multiplier for L channel
    sim_freq_l: f32,
    /// Simulation frequency multiplier for R channel
    sim_freq_r: f32,
    /// Base activity level (simulates quiet vs loud moments)
    activity_level: f32,
    /// Activity change timer
    activity_timer: u32,
}

impl Default for TrackState {
    fn default() -> Self {
        // Use different random frequencies for L and R channels
        Self {
            peak_hold_l: 0.0,
            peak_hold_r: 0.0,
            rms_history_l: Vec::with_capacity(8),
            rms_history_r: Vec::with_capacity(8),
            sim_phase_l: rand::random::<f32>() * std::f32::consts::TAU,
            sim_phase_r: rand::random::<f32>() * std::f32::consts::TAU,
            sim_freq_l: 0.8 + (rand::random::<f32>() * 1.2),
            sim_freq_r: 0.8 + (rand::random::<f32>() * 1.2),
            activity_level: 0.3 + rand::random::<f32>() * 0.4,
            activity_timer: 0,
        }
    }
}

/// Audio level monitoring service
pub struct AudioLevelService {
    /// Running state
    running: Arc<AtomicBool>,
    /// Tracked source IDs and their simulated/real levels
    tracked_sources: Arc<Mutex<HashMap<String, f32>>>,
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
            sources.entry(id.clone()).or_insert(0.0);
            states.entry(id).or_insert_with(TrackState::default);
        }
    }

    /// Update the level for a specific source (called from audio capture)
    pub async fn update_source_level(&self, source_id: &str, rms: f32) {
        let mut sources = self.tracked_sources.lock().await;
        if let Some(level) = sources.get_mut(source_id) {
            *level = rms.clamp(0.0, 1.0);
        }
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
            log::info!("AudioLevelService started");

            // ~30Hz update rate for smooth meters
            let mut ticker = interval(Duration::from_millis(33));

            while running.load(Ordering::Relaxed) {
                ticker.tick().await;

                let sources = tracked_sources.lock().await;
                let mut states = track_states.lock().await;

                if sources.is_empty() {
                    // No sources to monitor, still emit with just master
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

                for (source_id, &raw_level) in sources.iter() {
                    let state = states.entry(source_id.clone()).or_insert_with(TrackState::default);

                    // Update activity level periodically (simulates talking vs silence)
                    state.activity_timer += 1;
                    if state.activity_timer > 30 + (rand::random::<u32>() % 60) {
                        state.activity_timer = 0;
                        // Randomly shift activity level
                        let shift = (rand::random::<f32>() - 0.5) * 0.3;
                        state.activity_level = (state.activity_level + shift).clamp(0.1, 0.8);
                    }

                    // Generate truly independent L/R channels
                    let (level_l, level_r) = if raw_level < 0.001 {
                        // Advance L channel phase
                        state.sim_phase_l += 0.15 * state.sim_freq_l;
                        if state.sim_phase_l > std::f32::consts::TAU {
                            state.sim_phase_l -= std::f32::consts::TAU;
                        }

                        // Advance R channel phase (independent)
                        state.sim_phase_r += 0.15 * state.sim_freq_r;
                        if state.sim_phase_r > std::f32::consts::TAU {
                            state.sim_phase_r -= std::f32::consts::TAU;
                        }

                        // L channel: sine wave + noise + occasional bursts
                        let base_l = (state.sim_phase_l.sin() * 0.5 + 0.5) * state.activity_level;
                        let noise_l = rand::random::<f32>() * 0.12;
                        let burst_l = if rand::random::<f32>() > 0.97 { rand::random::<f32>() * 0.25 } else { 0.0 };
                        let raw_l = (base_l + noise_l + burst_l).min(0.98);

                        // R channel: completely independent calculation
                        let base_r = (state.sim_phase_r.sin() * 0.5 + 0.5) * state.activity_level;
                        let noise_r = rand::random::<f32>() * 0.12;
                        let burst_r = if rand::random::<f32>() > 0.97 { rand::random::<f32>() * 0.25 } else { 0.0 };
                        let raw_r = (base_r + noise_r + burst_r).min(0.98);

                        (raw_l, raw_r)
                    } else {
                        // Real audio - use same level for both until we get real stereo
                        (raw_level, raw_level)
                    };

                    // Smooth L channel RMS
                    state.rms_history_l.push(level_l);
                    if state.rms_history_l.len() > 6 {
                        state.rms_history_l.remove(0);
                    }
                    let smoothed_rms_l = state.rms_history_l.iter().sum::<f32>() / state.rms_history_l.len() as f32;

                    // Smooth R channel RMS
                    state.rms_history_r.push(level_r);
                    if state.rms_history_r.len() > 6 {
                        state.rms_history_r.remove(0);
                    }
                    let smoothed_rms_r = state.rms_history_r.iter().sum::<f32>() / state.rms_history_r.len() as f32;

                    // Combined RMS for overall level
                    let smoothed_rms = ((smoothed_rms_l.powi(2) + smoothed_rms_r.powi(2)) / 2.0).sqrt();

                    // Peak hold for L channel
                    if level_l > state.peak_hold_l {
                        state.peak_hold_l = level_l;
                    } else {
                        state.peak_hold_l = (state.peak_hold_l * 0.98).max(level_l * 0.5);
                    }

                    // Peak hold for R channel
                    if level_r > state.peak_hold_r {
                        state.peak_hold_r = level_r;
                    } else {
                        state.peak_hold_r = (state.peak_hold_r * 0.98).max(level_r * 0.5);
                    }

                    // Overall peak
                    let overall_peak = state.peak_hold_l.max(state.peak_hold_r);
                    let clipping = overall_peak > 0.95;

                    // Calculate dB for display
                    let peak_db = if overall_peak > 0.0 {
                        Some(20.0 * overall_peak.log10())
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

                    // Accumulate for master (separate L/R channels)
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

                // Calculate master RMS (sum of squares, then sqrt) - overall and per-channel
                let num_sources = sources.len() as f32;
                let master_rms = (master_rms_sum / num_sources).sqrt();
                let master_rms_l = (master_rms_l_sum / num_sources).sqrt();
                let master_rms_r = (master_rms_r_sum / num_sources).sqrt();

                // Calculate master dB
                let master_peak_db = if master_peak > 0.0 {
                    Some(20.0 * master_peak.log10())
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
