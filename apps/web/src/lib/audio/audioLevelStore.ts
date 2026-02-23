/**
 * Pure JavaScript Audio Level Store
 *
 * This store completely decouples audio level data from React's render cycle.
 * Updates are done via direct mutation for zero-allocation performance.
 *
 * Data flow:
 * WebSocket event → updateLevels() [pure JS] → RAF loop reads from store → canvas.draw()
 *
 * This eliminates ~30 React re-renders per second that were causing UI jank.
 */

export interface StereoLevel {
  rms: number;
  peak: number;
  leftRms: number;
  leftPeak: number;
  rightRms: number;
  rightPeak: number;
  clipping: boolean;
  peakDb: number;
  /** Pre-fader peak L (for gain staging display). Only present when mixer is active. */
  inputPeakL?: number;
  /** Pre-fader peak R (for gain staging display). Only present when mixer is active. */
  inputPeakR?: number;
}

/** Incoming data format from WebSocket */
export interface AudioLevelsData {
  tracks: Record<string, {
    rms: number;
    peak: number;
    clipping: boolean;
    leftRms?: number;
    leftPeak?: number;
    rightRms?: number;
    rightPeak?: number;
    peakDb?: number;
    inputPeakL?: number;
    inputPeakR?: number;
  }>;
  master: {
    rms: number;
    peak: number;
    clipping: boolean;
    leftRms?: number;
    leftPeak?: number;
    rightRms?: number;
    rightPeak?: number;
    peakDb?: number;
  };
}

interface PeakHoldState {
  peakHoldL: number;
  peakHoldR: number;
  peakHoldTime: number;
}

interface AudioLevelsState {
  tracks: Record<string, StereoLevel>;
  master: StereoLevel;
  peakHolds: Record<string, PeakHoldState>;
  masterPeakHold: PeakHoldState;
  version: number; // Incremented on each update for dirty checking
}

const DEFAULT_LEVEL: StereoLevel = {
  rms: 0,
  peak: 0,
  leftRms: 0,
  leftPeak: 0,
  rightRms: 0,
  rightPeak: 0,
  clipping: false,
  peakDb: -Infinity,
};

const DEFAULT_PEAK_HOLD: PeakHoldState = {
  peakHoldL: 0,
  peakHoldR: 0,
  peakHoldTime: 0,
};

// Singleton state - mutated in place, never triggers React re-renders
const state: AudioLevelsState = {
  tracks: {},
  master: { ...DEFAULT_LEVEL },
  peakHolds: {},
  masterPeakHold: { ...DEFAULT_PEAK_HOLD },
  version: 0,
};

import { PEAK_HOLD_DURATION_MS, peakDecayFactor } from './meterConstants';
import { decodeAudioFrame } from './binaryCodec';

// OBS-parity peak decay: 6.92 dB/s, dB-linear.
// Store runs at ~60fps (RAF callback rate).
const PEAK_DECAY_FACTOR = peakDecayFactor(60);

/**
 * Convert linear amplitude to dB
 */
function linearToDb(linear: number): number {
  if (linear <= 0) return -Infinity;
  return 20 * Math.log10(linear);
}

/**
 * Update peak hold state with OBS-parity behavior:
 * - Hold at max for 20 seconds
 * - Then dB-linear decay at 6.92 dB/s (OBS PPM: 11.76 dB / 1.7s)
 */
function updatePeakHold(
  peakState: PeakHoldState,
  leftPeak: number,
  rightPeak: number,
  now: number
): void {
  // Left channel
  if (leftPeak > peakState.peakHoldL) {
    peakState.peakHoldL = leftPeak;
    peakState.peakHoldTime = now;
  } else if (now - peakState.peakHoldTime > PEAK_HOLD_DURATION_MS) {
    // dB-linear decay (multiplicative in amplitude domain)
    peakState.peakHoldL *= PEAK_DECAY_FACTOR;
    if (peakState.peakHoldL < 0.001) peakState.peakHoldL = 0;
  }

  // Right channel
  if (rightPeak > peakState.peakHoldR) {
    peakState.peakHoldR = rightPeak;
    peakState.peakHoldTime = now;
  } else if (now - peakState.peakHoldTime > PEAK_HOLD_DURATION_MS) {
    peakState.peakHoldR *= PEAK_DECAY_FACTOR;
    if (peakState.peakHoldR < 0.001) peakState.peakHoldR = 0;
  }
}

/**
 * Update levels from WebSocket data.
 * This is called from the WebSocket handler and bypasses React entirely.
 * The data is mutated in place for zero-allocation updates.
 */
export function updateLevels(data: AudioLevelsData): void {
  const now = performance.now();

  // Update master level
  const masterData = data.master;
  state.master.rms = masterData.rms;
  state.master.peak = masterData.peak;
  state.master.clipping = masterData.clipping;
  state.master.leftRms = masterData.leftRms ?? masterData.rms;
  state.master.leftPeak = masterData.leftPeak ?? masterData.peak;
  state.master.rightRms = masterData.rightRms ?? masterData.rms;
  state.master.rightPeak = masterData.rightPeak ?? masterData.peak;
  state.master.peakDb = masterData.peakDb ?? linearToDb(masterData.peak);

  // Update master peak hold
  updatePeakHold(
    state.masterPeakHold,
    state.master.leftPeak,
    state.master.rightPeak,
    now
  );

  // Update each track
  for (const [id, trackData] of Object.entries(data.tracks)) {
    // Create track if it doesn't exist
    if (!state.tracks[id]) {
      state.tracks[id] = { ...DEFAULT_LEVEL };
    }
    if (!state.peakHolds[id]) {
      state.peakHolds[id] = { ...DEFAULT_PEAK_HOLD };
    }

    const track = state.tracks[id];
    track.rms = trackData.rms;
    track.peak = trackData.peak;
    track.clipping = trackData.clipping;
    track.leftRms = trackData.leftRms ?? trackData.rms;
    track.leftPeak = trackData.leftPeak ?? trackData.peak;
    track.rightRms = trackData.rightRms ?? trackData.rms;
    track.rightPeak = trackData.rightPeak ?? trackData.peak;
    track.peakDb = trackData.peakDb ?? linearToDb(trackData.peak);
    track.inputPeakL = trackData.inputPeakL;
    track.inputPeakR = trackData.inputPeakR;

    // Update track peak hold
    updatePeakHold(state.peakHolds[id], track.leftPeak, track.rightPeak, now);
  }

  // Clean up tracks that are no longer in the data
  for (const id of Object.keys(state.tracks)) {
    if (!(id in data.tracks)) {
      delete state.tracks[id];
      delete state.peakHolds[id];
    }
  }

  // Increment version for dirty checking in RAF loops
  state.version++;
}

/**
 * Update levels from a binary WebSocket frame.
 * This is the fast path — decodes the compact binary format directly
 * into the store with zero JSON parsing overhead.
 */
export function updateLevelsBinary(buffer: ArrayBuffer): void {
  const frame = decodeAudioFrame(buffer);
  if (!frame) return;

  const now = performance.now();
  const m = frame.master;

  // Update master level
  state.master.rms = m.rms;
  state.master.peak = m.peak;
  state.master.clipping = m.clipping;
  state.master.leftRms = m.rmsL;
  state.master.leftPeak = m.peakL;
  state.master.rightRms = m.rmsR;
  state.master.rightPeak = m.peakR;
  state.master.peakDb = m.peakDb;

  // Update master peak hold
  updatePeakHold(state.masterPeakHold, m.peakL, m.peakR, now);

  // Track IDs we've seen in this frame (for cleanup)
  const seenIds = new Set<string>();

  // Update each track
  for (const [id, t] of frame.tracks) {
    seenIds.add(id);

    if (!state.tracks[id]) {
      state.tracks[id] = { ...DEFAULT_LEVEL };
    }
    if (!state.peakHolds[id]) {
      state.peakHolds[id] = { ...DEFAULT_PEAK_HOLD };
    }

    const track = state.tracks[id];
    track.rms = t.rms;
    track.peak = t.peak;
    track.clipping = t.clipping;
    track.leftRms = t.rmsL;
    track.leftPeak = t.peakL;
    track.rightRms = t.rmsR;
    track.rightPeak = t.peakR;
    track.peakDb = t.peakDb;
    track.inputPeakL = t.hasInputPeak ? t.inputPeakL : undefined;
    track.inputPeakR = t.hasInputPeak ? t.inputPeakR : undefined;

    updatePeakHold(state.peakHolds[id], t.peakL, t.peakR, now);
  }

  // Clean up tracks no longer present
  for (const id of Object.keys(state.tracks)) {
    if (!seenIds.has(id)) {
      delete state.tracks[id];
      delete state.peakHolds[id];
    }
  }

  state.version++;
}

/**
 * Get audio level for a specific track.
 * Returns a reference to the internal state object (read-only).
 */
export function getTrackLevel(sourceId: string): StereoLevel {
  return state.tracks[sourceId] ?? DEFAULT_LEVEL;
}

/**
 * Get master audio level.
 * Returns a reference to the internal state object (read-only).
 */
export function getMasterLevel(): StereoLevel {
  return state.master;
}

/**
 * Get peak hold values for a track.
 * Returns { left, right } peak hold levels.
 */
export function getPeakHold(sourceId: string): { left: number; right: number } {
  const peakState = state.peakHolds[sourceId];
  if (!peakState) {
    return { left: 0, right: 0 };
  }
  return {
    left: peakState.peakHoldL,
    right: peakState.peakHoldR,
  };
}

/**
 * Get master peak hold values.
 */
export function getMasterPeakHold(): { left: number; right: number } {
  return {
    left: state.masterPeakHold.peakHoldL,
    right: state.masterPeakHold.peakHoldR,
  };
}

/**
 * Get the current version number.
 * Used for dirty checking - if version hasn't changed, no need to redraw.
 */
export function getVersion(): number {
  return state.version;
}

/**
 * Reset all levels to zero.
 * Called when WebSocket disconnects.
 */
export function resetLevels(): void {
  state.master = { ...DEFAULT_LEVEL };
  state.masterPeakHold = { ...DEFAULT_PEAK_HOLD };
  state.tracks = {};
  state.peakHolds = {};
  state.version++;
}
