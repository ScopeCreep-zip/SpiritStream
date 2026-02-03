/**
 * Meter Renderer
 *
 * Pure canvas drawing functions for audio level meters.
 * Extracted from UnifiedChannelStrip to enable RAF-based rendering
 * that's decoupled from React's render cycle.
 */

import type { StereoLevel } from './audioLevelStore';
import type { AudioFilter } from '@/types/source';

// Meter dimensions (constants for consistency)
export const LABEL_WIDTH = 22;
export const BAR_WIDTH = 28;
export const ARROW_WIDTH = 8;
export const ARROW_HEIGHT = 12;
export const PADDING_Y = 8;
export const METER_HEIGHT = 160;
export const TOTAL_HEIGHT = METER_HEIGHT + PADDING_Y * 2;
export const TOTAL_WIDTH = LABEL_WIDTH + BAR_WIDTH + ARROW_WIDTH;

// Threshold marker colors
const THRESHOLD_COLORS: Record<string, string> = {
  noiseGate: '#a855f7',    // Purple
  compressor: '#3b82f6',   // Blue
  expander: '#f59e0b',     // Amber
};

// Cached gradient - reused across all meter renders
let cachedGradient: CanvasGradient | null = null;
let cachedGradientCanvas: CanvasRenderingContext2D | null = null;

/**
 * Convert dB to visual position (percentage from bottom)
 * Linear dB scale: 0dB = 100%, -60dB = 0%
 */
function dbToPosition(db: number): number {
  const clampedDb = Math.max(-60, Math.min(0, db));
  return ((clampedDb + 60) / 60) * 100;
}

/**
 * Convert linear amplitude (0-1) to visual position
 */
function linearToPosition(linear: number): number {
  if (linear <= 0) return 0;
  const db = 20 * Math.log10(linear);
  return dbToPosition(db);
}

/**
 * Get or create the gradient for meter fill
 */
function getGradient(ctx: CanvasRenderingContext2D, meterTop: number, meterBottom: number): CanvasGradient {
  // Recreate gradient if context changed (different canvas)
  if (!cachedGradient || cachedGradientCanvas !== ctx) {
    cachedGradient = ctx.createLinearGradient(0, meterBottom, 0, meterTop);
    cachedGradient.addColorStop(0, '#166534');      // Muted green at -60dB
    cachedGradient.addColorStop(0.67, '#15803d');   // Green up to -20dB
    cachedGradient.addColorStop(0.67, '#a16207');   // Muted yellow starts at -20dB
    cachedGradient.addColorStop(0.90, '#ca8a04');   // Yellow up to -6dB
    cachedGradient.addColorStop(0.90, '#c2410c');   // Muted orange starts at -6dB
    cachedGradient.addColorStop(0.95, '#ea580c');   // Orange up to -3dB
    cachedGradient.addColorStop(0.95, '#b91c1c');   // Muted red starts at -3dB
    cachedGradient.addColorStop(1, '#dc2626');      // Red at 0dB
    cachedGradientCanvas = ctx;
  }
  return cachedGradient;
}

export interface MeterRenderOptions {
  /** Audio level data */
  level: StereoLevel;
  /** Peak hold values { left, right } */
  peakHold: { left: number; right: number };
  /** Current volume (0-1) */
  volume: number;
  /** Is the track muted */
  muted: boolean;
  /** Is the volume being dragged */
  isDragging: boolean;
  /** Active filters with thresholds */
  thresholdFilters: AudioFilter[];
}

/**
 * Draw the complete audio meter.
 * This is called from a RAF loop and reads directly from the audio level store.
 */
export function drawMeter(
  ctx: CanvasRenderingContext2D,
  options: MeterRenderOptions
): void {
  const { level, peakHold, volume, muted, isDragging, thresholdFilters } = options;

  // Meter coordinates
  const meterX = LABEL_WIDTH;
  const meterTop = PADDING_Y;
  const meterBottom = PADDING_Y + METER_HEIGHT;

  // Helper to convert dB position (0-100%) to Y coordinate
  const dbToY = (position: number) => meterBottom - (position / 100) * METER_HEIGHT;

  // Clear canvas
  ctx.clearRect(0, 0, TOTAL_WIDTH, TOTAL_HEIGHT);

  // 1. Background fill (dark)
  ctx.fillStyle = 'rgba(0, 0, 0, 0.5)';
  ctx.beginPath();
  ctx.roundRect(meterX, meterTop, BAR_WIDTH, METER_HEIGHT, 4);
  ctx.fill();

  // Get gradient (cached)
  const gradient = getGradient(ctx, meterTop, meterBottom);

  // 2. VU level fill - stereo L/R bars
  const channelWidth = (BAR_WIDTH - 4) / 2 - 1; // 2px gap in middle
  const effectiveLeftPeak = muted ? 0 : level.leftPeak;
  const effectiveRightPeak = muted ? 0 : level.rightPeak;
  const effectiveLeftRms = muted ? 0 : level.leftRms;
  const effectiveRightRms = muted ? 0 : level.rightRms;

  // Left channel peak fill
  const leftPeakPosition = linearToPosition(effectiveLeftPeak);
  const leftPeakHeight = (leftPeakPosition / 100) * METER_HEIGHT;
  if (leftPeakHeight > 0) {
    ctx.fillStyle = gradient;
    ctx.beginPath();
    ctx.roundRect(meterX + 2, meterBottom - leftPeakHeight, channelWidth, leftPeakHeight, [0, 0, 2, 2]);
    ctx.fill();
  }

  // Right channel peak fill
  const rightX = meterX + 2 + channelWidth + 2;
  const rightPeakPosition = linearToPosition(effectiveRightPeak);
  const rightPeakHeight = (rightPeakPosition / 100) * METER_HEIGHT;
  if (rightPeakHeight > 0) {
    ctx.fillStyle = gradient;
    ctx.beginPath();
    ctx.roundRect(rightX, meterBottom - rightPeakHeight, channelWidth, rightPeakHeight, [0, 0, 2, 2]);
    ctx.fill();
  }

  // 3. RMS lines on top of peak fill
  const leftRmsPosition = linearToPosition(effectiveLeftRms);
  if (leftRmsPosition > 0 && effectiveLeftPeak > 0) {
    const leftRmsY = dbToY(leftRmsPosition);
    ctx.fillStyle = '#000000';
    ctx.fillRect(meterX + 2, leftRmsY - 2, channelWidth, 4);
    ctx.fillStyle = '#ffffff';
    ctx.fillRect(meterX + 2, leftRmsY - 1, channelWidth, 2);
  }

  const rightRmsPosition = linearToPosition(effectiveRightRms);
  if (rightRmsPosition > 0 && effectiveRightPeak > 0) {
    const rightRmsY = dbToY(rightRmsPosition);
    ctx.fillStyle = '#000000';
    ctx.fillRect(rightX, rightRmsY - 2, channelWidth, 4);
    ctx.fillStyle = '#ffffff';
    ctx.fillRect(rightX, rightRmsY - 1, channelWidth, 2);
  }

  // 4. L/R labels at top
  ctx.font = '6px system-ui, sans-serif';
  ctx.textAlign = 'center';
  ctx.fillStyle = 'rgba(255, 255, 255, 0.7)';
  ctx.fillText('L', meterX + 2 + channelWidth / 2, meterTop + 8);
  ctx.fillText('R', rightX + channelWidth / 2, meterTop + 8);

  // 5. Stereo separator line
  const separatorX = meterX + 2 + channelWidth + 1;
  ctx.fillStyle = 'rgba(255, 255, 255, 0.5)';
  ctx.fillRect(separatorX - 0.5, meterTop, 1, METER_HEIGHT);

  // 6. dB scale markers
  ctx.font = '8px system-ui, sans-serif';
  ctx.textAlign = 'right';
  ctx.textBaseline = 'middle';

  const labelMarkers = [0, -5, -10, -15, -20, -25, -30, -35, -40, -45, -50, -55, -60];
  for (const db of labelMarkers) {
    const position = dbToPosition(db);
    const y = dbToY(position);

    // Soft tick across meter
    ctx.fillStyle = 'rgba(255, 255, 255, 0.2)';
    ctx.fillRect(meterX + 1, y - 0.5, BAR_WIDTH - 2, 1);

    // Tick extending left
    ctx.fillStyle = 'rgba(255, 255, 255, 0.4)';
    ctx.fillRect(meterX - 4, y - 0.5, 4, 1);

    // dB label
    ctx.fillStyle = 'rgba(255, 255, 255, 0.6)';
    const label = db === 0 ? '0' : String(db);
    ctx.fillText(label, meterX - 5, y);
  }

  // 7. Reference level markers (-20dB and -9dB)
  const referenceMarkers = [
    { db: -20, color: 'rgba(34, 197, 94, 0.9)' },
    { db: -9, color: 'rgba(234, 179, 8, 0.9)' },
  ];
  for (const { db, color } of referenceMarkers) {
    const position = dbToPosition(db);
    const y = dbToY(position);
    ctx.fillStyle = color;
    ctx.fillRect(meterX + 1, y - 1, BAR_WIDTH - 2, 2);
  }

  // 8. Threshold markers for active filters
  for (const filter of thresholdFilters) {
    const threshold = (filter as { threshold?: number }).threshold ?? -40;
    const position = dbToPosition(threshold);
    const y = dbToY(position);
    const color = THRESHOLD_COLORS[filter.type] || '#888';

    ctx.strokeStyle = color;
    ctx.lineWidth = 2;
    ctx.setLineDash([4, 3]);
    ctx.beginPath();
    ctx.moveTo(meterX, y);
    ctx.lineTo(meterX + BAR_WIDTH, y);
    ctx.stroke();
    ctx.setLineDash([]);
  }

  // 9. Peak hold line - uses combined max of L/R
  const effectivePeakHoldL = muted ? 0 : peakHold.left;
  const effectivePeakHoldR = muted ? 0 : peakHold.right;
  const maxPeakHold = Math.max(effectivePeakHoldL, effectivePeakHoldR);

  if (maxPeakHold > 0.01) {
    const peakPosition = linearToPosition(maxPeakHold);
    const peakY = dbToY(peakPosition);

    // Color based on dB level
    const peakDbVal = maxPeakHold > 0 ? 20 * Math.log10(maxPeakHold) : -60;
    let peakColor = '#22c55e';  // Green (below -20dB)
    if (peakDbVal > -3) peakColor = '#ef4444';       // Red (above -3dB)
    else if (peakDbVal > -6) peakColor = '#f97316';  // Orange (-6 to -3dB)
    else if (peakDbVal > -20) peakColor = '#eab308'; // Yellow (-20 to -6dB)

    ctx.fillStyle = peakColor;
    ctx.fillRect(meterX + 2, peakY - 1.5, BAR_WIDTH - 4, 3);
  }

  // 10. Volume arrow
  const volumeY = meterBottom - volume * METER_HEIGHT;
  const arrowX = meterX + BAR_WIDTH;

  ctx.beginPath();
  ctx.moveTo(arrowX, volumeY);
  ctx.lineTo(arrowX + ARROW_WIDTH, volumeY - ARROW_HEIGHT / 2);
  ctx.lineTo(arrowX + ARROW_WIDTH, volumeY + ARROW_HEIGHT / 2);
  ctx.closePath();

  if (isDragging) {
    ctx.fillStyle = '#7C3AED';
    ctx.fill();
  } else {
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.9)';
    ctx.lineWidth = 2;
    ctx.stroke();
  }
}
