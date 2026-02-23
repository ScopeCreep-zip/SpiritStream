/**
 * Shared constants for audio meter rendering.
 *
 * Single source of truth for layout dimensions, OBS-parity peak decay,
 * and threshold marker colors. Consumed by:
 * - audioLevelStore.ts (peak hold decay)
 * - meterRenderer.ts (canvas drawing)
 * - meterCoordinator.ts (RAF loop timing)
 */

// ============================================================================
// Layout dimensions (must stay in sync across all renderers)
// ============================================================================
export const LABEL_WIDTH = 22;
export const BAR_WIDTH = 28;
export const ARROW_WIDTH = 8;
export const ARROW_HEIGHT = 12;
export const PADDING_Y = 8;
export const METER_HEIGHT = 160;
export const TOTAL_HEIGHT = METER_HEIGHT + PADDING_Y * 2;
export const TOTAL_WIDTH = LABEL_WIDTH + BAR_WIDTH + ARROW_WIDTH;

// ============================================================================
// OBS-parity peak hold / decay
// ============================================================================

/** Peak hold duration before decay starts (OBS: 20 seconds) */
export const PEAK_HOLD_DURATION_MS = 20000;

/**
 * OBS PPM peak decay rate: 11.76 dB over 1.7 seconds = 6.92 dB/s.
 * This is linear in the dB domain (not linear in amplitude).
 */
export const PEAK_DECAY_DB_PER_SEC = 6.92;

/**
 * Compute the multiplicative decay factor to apply per frame.
 * Converts dB-linear decay to an amplitude multiplier.
 *
 * @param fps - Frames per second of the rendering/update loop
 * @returns Amplitude multiplier to apply each frame during decay
 */
export function peakDecayFactor(fps: number): number {
  // decay_db_per_frame = PEAK_DECAY_DB_PER_SEC / fps
  // amplitude_factor = 10^(-decay_db_per_frame / 20)
  return Math.pow(10, -(PEAK_DECAY_DB_PER_SEC / fps) / 20);
}

// ============================================================================
// Threshold marker colors
// ============================================================================
export const THRESHOLD_COLORS: Record<string, string> = {
  noiseGate: '#a855f7',    // Purple
  compressor: '#3b82f6',   // Blue
  expander: '#f59e0b',     // Amber
};
