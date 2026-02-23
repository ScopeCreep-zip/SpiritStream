// Noise Gate Filter
// 5-state machine: Closed → Attack → Open → Hold → Release → Closed
// OBS reference: plugins/obs-filters/noise-gate-filter.c

use super::AudioFilter;

#[derive(Debug, Clone, Copy, PartialEq)]
enum GateState {
    Closed,
    Attack,
    Open,
    Hold,
    Release,
}

pub struct NoiseGateFilter {
    open_threshold_db: f32,
    close_threshold_db: f32,
    attack_ms: f32,
    hold_ms: f32,
    release_ms: f32,
    state: GateState,
    /// Current gate gain (0.0 = closed, 1.0 = open)
    gate_gain: f32,
    /// Samples remaining in hold state
    hold_counter: usize,
    /// Cached coefficients
    attack_rate: f32,
    release_rate: f32,
    hold_samples: usize,
    cached_sample_rate: u32,
}

impl NoiseGateFilter {
    pub fn new(
        open_threshold_db: f32,
        close_threshold_db: f32,
        attack_ms: f32,
        hold_ms: f32,
        release_ms: f32,
    ) -> Self {
        Self {
            open_threshold_db,
            close_threshold_db,
            attack_ms,
            hold_ms,
            release_ms,
            state: GateState::Closed,
            gate_gain: 0.0,
            hold_counter: 0,
            attack_rate: 0.0,
            release_rate: 0.0,
            hold_samples: 0,
            cached_sample_rate: 0,
        }
    }

    fn update_coefficients(&mut self, sample_rate: u32) {
        if sample_rate == self.cached_sample_rate {
            return;
        }
        self.cached_sample_rate = sample_rate;
        let sr = sample_rate as f32;
        // Rate of gain change per sample during attack/release
        let attack_samples = (self.attack_ms * 0.001 * sr).max(1.0);
        let release_samples = (self.release_ms * 0.001 * sr).max(1.0);
        self.attack_rate = 1.0 / attack_samples;
        self.release_rate = 1.0 / release_samples;
        self.hold_samples = (self.hold_ms * 0.001 * sr) as usize;
    }
}

impl AudioFilter for NoiseGateFilter {
    fn process(&mut self, samples: &mut [f32], channels: usize, sample_rate: u32) {
        self.update_coefficients(sample_rate);
        let ch = channels.max(1);

        for frame in samples.chunks_mut(ch) {
            let peak = frame.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
            let level_db = linear_to_db(peak);

            // State machine transitions
            match self.state {
                GateState::Closed => {
                    if level_db >= self.open_threshold_db {
                        self.state = GateState::Attack;
                    }
                }
                GateState::Attack => {
                    self.gate_gain += self.attack_rate;
                    if self.gate_gain >= 1.0 {
                        self.gate_gain = 1.0;
                        self.state = GateState::Open;
                    }
                }
                GateState::Open => {
                    if level_db < self.close_threshold_db {
                        self.state = GateState::Hold;
                        self.hold_counter = self.hold_samples;
                    }
                }
                GateState::Hold => {
                    if level_db >= self.open_threshold_db {
                        self.state = GateState::Open;
                    } else if self.hold_counter == 0 {
                        self.state = GateState::Release;
                    } else {
                        self.hold_counter -= 1;
                    }
                }
                GateState::Release => {
                    if level_db >= self.open_threshold_db {
                        self.state = GateState::Attack;
                    } else {
                        self.gate_gain -= self.release_rate;
                        if self.gate_gain <= 0.0 {
                            self.gate_gain = 0.0;
                            self.state = GateState::Closed;
                        }
                    }
                }
            }

            for s in frame.iter_mut() {
                *s *= self.gate_gain;
            }
        }
    }

    fn reset(&mut self) {
        self.state = GateState::Closed;
        self.gate_gain = 0.0;
        self.hold_counter = 0;
    }

    fn filter_type(&self) -> &str {
        "noise_gate"
    }
}

fn linear_to_db(linear: f32) -> f32 {
    if linear > 1e-6 {
        20.0 * linear.log10()
    } else {
        -96.0
    }
}
