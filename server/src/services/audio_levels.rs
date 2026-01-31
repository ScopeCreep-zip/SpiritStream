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
pub struct AudioLevel {
    /// RMS level (0.0 - 1.0)
    pub rms: f32,
    /// Peak level (0.0 - 1.0)
    pub peak: f32,
    /// Whether clipping was detected
    pub clipping: bool,
}

impl Default for AudioLevel {
    fn default() -> Self {
        Self {
            rms: 0.0,
            peak: 0.0,
            clipping: false,
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
    /// Recent peak samples for decay
    peak_hold: f32,
    /// Peak decay counter
    peak_decay_counter: u32,
    /// Recent RMS samples for smoothing
    rms_history: Vec<f32>,
    /// Simulation phase for generating test audio
    sim_phase: f32,
    /// Simulation frequency multiplier (unique per track)
    sim_freq: f32,
}

impl Default for TrackState {
    fn default() -> Self {
        // Use random frequency multiplier for variation
        let sim_freq = 0.5 + (rand::random::<f32>() * 1.5);
        Self {
            peak_hold: 0.0,
            peak_decay_counter: 0,
            rms_history: Vec::with_capacity(10),
            sim_phase: rand::random::<f32>() * std::f32::consts::TAU,
            sim_freq,
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
                let mut master_peak = 0.0f32;
                let mut master_clipping = false;

                for (source_id, &raw_level) in sources.iter() {
                    let state = states.entry(source_id.clone()).or_insert_with(TrackState::default);

                    // If no real audio, generate simulated levels for visualization
                    let effective_level = if raw_level < 0.001 {
                        // Generate simulated audio activity
                        state.sim_phase += 0.1 * state.sim_freq;
                        if state.sim_phase > std::f32::consts::TAU {
                            state.sim_phase -= std::f32::consts::TAU;
                        }

                        // Create a natural-looking audio pattern with some randomness
                        let base = (state.sim_phase.sin() * 0.5 + 0.5) * 0.4; // 0-0.4 base
                        let variation = rand::random::<f32>() * 0.15; // Add noise
                        let bursts = if rand::random::<f32>() > 0.95 { 0.3 } else { 0.0 }; // Occasional peaks
                        (base + variation + bursts).min(0.95)
                    } else {
                        raw_level
                    };

                    // Smooth RMS using history
                    state.rms_history.push(effective_level);
                    if state.rms_history.len() > 5 {
                        state.rms_history.remove(0);
                    }
                    let smoothed_rms = state.rms_history.iter().sum::<f32>() / state.rms_history.len() as f32;

                    // Peak hold with decay
                    if effective_level > state.peak_hold {
                        state.peak_hold = effective_level;
                        state.peak_decay_counter = 45; // ~1.5 seconds at 30Hz
                    } else if state.peak_decay_counter > 0 {
                        state.peak_decay_counter -= 1;
                    } else {
                        // Decay peak towards current level
                        state.peak_hold = (state.peak_hold * 0.95).max(effective_level);
                    }

                    let clipping = state.peak_hold > 0.95;

                    tracks.insert(source_id.clone(), AudioLevel {
                        rms: smoothed_rms,
                        peak: state.peak_hold,
                        clipping,
                    });

                    // Accumulate for master
                    master_rms_sum += smoothed_rms * smoothed_rms; // RMS is root-mean-square
                    master_peak = master_peak.max(state.peak_hold);
                    if clipping {
                        master_clipping = true;
                    }
                }

                // Calculate master RMS (sum of squares, then sqrt)
                let master_rms = (master_rms_sum / sources.len() as f32).sqrt();

                let data = AudioLevelsData {
                    tracks,
                    master: AudioLevel {
                        rms: master_rms,
                        peak: master_peak,
                        clipping: master_clipping,
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
