/**
 * Shared formatting utilities
 * Consolidates duplicated dB/volume formatting from UnifiedChannelStrip,
 * AudioMixerPanel, AdvancedAudioProperties, and useAudioLevels
 */

/** Format a dB value with sign and 1 decimal. Returns '-∞' for silence. */
export function formatDb(db: number): string {
  if (db <= -60 || !isFinite(db)) return '-\u221E';
  return db.toFixed(1);
}

/** Convert dB value to linear amplitude (0-1 range for unity gain) */
export function dbToLinear(db: number): number {
  return Math.pow(10, db / 20);
}

/** Convert linear amplitude to dB. Returns -Infinity for zero/negative. */
export function linearToDb(linear: number): number {
  if (linear <= 0) return -Infinity;
  return 20 * Math.log10(linear);
}

/** Get color for a given audio level (0-1 linear) */
export function getLevelColor(level: number): string {
  if (level > 0.9) return '#ef4444'; // Red - clipping danger
  if (level > 0.7) return '#f97316'; // Orange - warning
  if (level > 0.5) return '#eab308'; // Yellow - nominal high
  return '#22c55e'; // Green - safe
}
