/**
 * Audio Meter Worker
 *
 * Unified Web Worker that handles all audio level processing and meter rendering.
 * Runs completely off the main thread for zero-jank audio visualization.
 *
 * Features:
 * - Receives audio level data via MessagePort (JSON parsing in worker)
 * - Updates SharedArrayBuffer for zero-copy level reads
 * - Draws all meter canvases via OffscreenCanvas (one RAF loop for all)
 * - Peak hold with 20s hold and decay (OBS parity)
 *
 * Messages:
 * - 'init': Initialize with SharedArrayBuffer
 * - 'registerCanvas': Register an OffscreenCanvas for a track
 * - 'unregisterCanvas': Remove a canvas
 * - 'audioData': Raw audio level JSON string
 * - 'updateConfig': Update rendering config (volume, muted, etc)
 */

// Type definitions
interface StereoLevel {
  rms: number;
  peak: number;
  leftRms: number;
  leftPeak: number;
  rightRms: number;
  rightPeak: number;
  clipping: boolean;
  peakDb: number;
}

/** Simplified filter type for threshold display in worker */
interface ThresholdFilter {
  type: string;
  threshold?: number;
  enabled?: boolean;
}

/** Threshold marker colors matching meterRenderer.ts */
const THRESHOLD_COLORS: Record<string, string> = {
  noiseGate: '#a855f7',    // Purple
  compressor: '#3b82f6',   // Blue
  expander: '#f59e0b',     // Amber
};

interface TrackConfig {
  volume: number;
  muted: boolean;
  isDragging?: boolean;
  thresholdFilters?: ThresholdFilter[];
}

interface RegisteredCanvas {
  canvas: OffscreenCanvas;
  ctx: OffscreenCanvasRenderingContext2D;
  trackId: string | null; // null = master
  config: TrackConfig;
  dpr: number; // Device pixel ratio
  // Drawing state
  gradient: CanvasGradient | null;
  lastLevel: StereoLevel | null;
}

interface AudioLevelsPayload {
  tracks: Record<string, {
    rms: number;
    peak: number;
    clipping: boolean;
    leftRms?: number;
    leftPeak?: number;
    rightRms?: number;
    rightPeak?: number;
    peakDb?: number;
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

// Constants matching meterRenderer.ts (MUST stay in sync)
const LABEL_WIDTH = 22;
const BAR_WIDTH = 28;
const ARROW_WIDTH = 8;
const ARROW_HEIGHT = 12;
const PADDING_Y = 8;
const METER_HEIGHT = 160;
const TOTAL_HEIGHT = METER_HEIGHT + PADDING_Y * 2;
const TOTAL_WIDTH = LABEL_WIDTH + BAR_WIDTH + ARROW_WIDTH;
const PEAK_HOLD_TIME = 20000; // 20 seconds
const PEAK_DECAY_RATE = 0.98;

// Worker state
const canvases = new Map<string, RegisteredCanvas>();
const trackLevels = new Map<string, StereoLevel>();
const peakHolds = new Map<string, { left: number; right: number; time: number }>();
let masterLevel: StereoLevel = createDefaultLevel();
let masterPeakHold = { left: 0, right: 0, time: 0 };
// Note: rafId removed - now using setInterval (intervalId) for background tab support
let lastDataTime = 0;
let sharedBuffer: SharedArrayBuffer | null = null;
let sharedView: Float32Array | null = null;

// SharedArrayBuffer layout:
// [version(1), masterRms(1), masterPeak(1), masterLeftRms(1), masterLeftPeak(1),
//  masterRightRms(1), masterRightPeak(1), masterPeakDb(1), masterClipping(1),
//  ...tracks (8 floats each)]
const MASTER_OFFSET = 1;
const TRACK_SIZE = 8;
const MAX_TRACKS = 16;

function createDefaultLevel(): StereoLevel {
  return {
    rms: 0,
    peak: 0,
    leftRms: 0,
    leftPeak: 0,
    rightRms: 0,
    rightPeak: 0,
    clipping: false,
    peakDb: -60,
  };
}

function dbFromLinear(linear: number): number {
  if (linear <= 0.001) return -60;
  return Math.max(-60, 20 * Math.log10(linear));
}

function updatePeakHold(
  trackId: string | null,
  leftPeak: number,
  rightPeak: number
): { left: number; right: number } {
  const key = trackId ?? '__master__';
  const now = performance.now();
  let hold = trackId ? peakHolds.get(key) : masterPeakHold;

  if (!hold) {
    hold = { left: 0, right: 0, time: now };
    if (trackId) {
      peakHolds.set(key, hold);
    } else {
      masterPeakHold = hold;
    }
  }

  // Update peak hold with 20s hold then decay
  if (leftPeak > hold.left) {
    hold.left = leftPeak;
    hold.time = now;
  } else if (now - hold.time > PEAK_HOLD_TIME) {
    hold.left *= PEAK_DECAY_RATE;
  }

  if (rightPeak > hold.right) {
    hold.right = rightPeak;
    hold.time = now;
  } else if (now - hold.time > PEAK_HOLD_TIME) {
    hold.right *= PEAK_DECAY_RATE;
  }

  return hold;
}

function processAudioData(jsonString: string): void {
  try {
    const data = JSON.parse(jsonString) as { event?: string; payload?: AudioLevelsPayload };
    if (data.event !== 'audio_levels' || !data.payload) return;

    const { tracks, master } = data.payload;
    lastDataTime = performance.now();

    // Update master level
    masterLevel = {
      rms: master.rms ?? 0,
      peak: master.peak ?? 0,
      leftRms: master.leftRms ?? master.rms ?? 0,
      leftPeak: master.leftPeak ?? master.peak ?? 0,
      rightRms: master.rightRms ?? master.rms ?? 0,
      rightPeak: master.rightPeak ?? master.peak ?? 0,
      clipping: master.clipping ?? false,
      peakDb: master.peakDb ?? dbFromLinear(master.peak ?? 0),
    };

    // Update track levels
    for (const [trackId, level] of Object.entries(tracks)) {
      trackLevels.set(trackId, {
        rms: level.rms ?? 0,
        peak: level.peak ?? 0,
        leftRms: level.leftRms ?? level.rms ?? 0,
        leftPeak: level.leftPeak ?? level.peak ?? 0,
        rightRms: level.rightRms ?? level.rms ?? 0,
        rightPeak: level.rightPeak ?? level.peak ?? 0,
        clipping: level.clipping ?? false,
        peakDb: level.peakDb ?? dbFromLinear(level.peak ?? 0),
      });
    }

    // Update SharedArrayBuffer if available
    if (sharedView) {
      sharedView[0]++; // Increment version

      // Master
      sharedView[MASTER_OFFSET + 0] = masterLevel.rms;
      sharedView[MASTER_OFFSET + 1] = masterLevel.peak;
      sharedView[MASTER_OFFSET + 2] = masterLevel.leftRms;
      sharedView[MASTER_OFFSET + 3] = masterLevel.leftPeak;
      sharedView[MASTER_OFFSET + 4] = masterLevel.rightRms;
      sharedView[MASTER_OFFSET + 5] = masterLevel.rightPeak;
      sharedView[MASTER_OFFSET + 6] = masterLevel.peakDb;
      sharedView[MASTER_OFFSET + 7] = masterLevel.clipping ? 1 : 0;

      // Tracks (limited to MAX_TRACKS)
      let trackIdx = 0;
      for (const [, level] of trackLevels) {
        if (trackIdx >= MAX_TRACKS) break;
        const offset = MASTER_OFFSET + 8 + trackIdx * TRACK_SIZE;
        sharedView[offset + 0] = level.rms;
        sharedView[offset + 1] = level.peak;
        sharedView[offset + 2] = level.leftRms;
        sharedView[offset + 3] = level.leftPeak;
        sharedView[offset + 4] = level.rightRms;
        sharedView[offset + 5] = level.rightPeak;
        sharedView[offset + 6] = level.peakDb;
        sharedView[offset + 7] = level.clipping ? 1 : 0;
        trackIdx++;
      }
    }
  } catch (err) {
    // Silently ignore parse errors
  }
}

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

function createGradient(ctx: OffscreenCanvasRenderingContext2D, meterTop: number, meterBottom: number): CanvasGradient {
  const gradient = ctx.createLinearGradient(0, meterBottom, 0, meterTop);
  // Bottom to top: green -> yellow -> orange -> red (matches meterRenderer.ts)
  gradient.addColorStop(0, '#166534');      // Muted green at -60dB
  gradient.addColorStop(0.67, '#15803d');   // Green up to -20dB
  gradient.addColorStop(0.67, '#a16207');   // Muted yellow starts at -20dB
  gradient.addColorStop(0.90, '#ca8a04');   // Yellow up to -6dB
  gradient.addColorStop(0.90, '#c2410c');   // Muted orange starts at -6dB
  gradient.addColorStop(0.95, '#ea580c');   // Orange up to -3dB
  gradient.addColorStop(0.95, '#b91c1c');   // Muted red starts at -3dB
  gradient.addColorStop(1, '#dc2626');      // Red at 0dB
  return gradient;
}

function drawMeter(registered: RegisteredCanvas): void {
  const { ctx, trackId, config } = registered;
  const { volume, muted, isDragging = false, thresholdFilters = [] } = config;
  const level = trackId ? trackLevels.get(trackId) : masterLevel;

  // Meter coordinates
  const meterX = LABEL_WIDTH;
  const meterTop = PADDING_Y;
  const meterBottom = PADDING_Y + METER_HEIGHT;

  // Helper to convert dB position (0-100%) to Y coordinate
  const dbToY = (position: number) => meterBottom - (position / 100) * METER_HEIGHT;

  // Clear canvas (use logical dimensions, ctx is already scaled by dpr)
  ctx.clearRect(0, 0, TOTAL_WIDTH, TOTAL_HEIGHT);

  if (!level) {
    // Draw empty meter background
    ctx.fillStyle = 'rgba(0, 0, 0, 0.5)';
    ctx.beginPath();
    ctx.roundRect(meterX, meterTop, BAR_WIDTH, METER_HEIGHT, 4);
    ctx.fill();
    return;
  }

  // Get or create gradient
  if (!registered.gradient) {
    registered.gradient = createGradient(ctx, meterTop, meterBottom);
  }

  const peakHold = updatePeakHold(trackId, level.leftPeak, level.rightPeak);

  // 1. Background fill (dark) with rounded corners
  ctx.fillStyle = 'rgba(0, 0, 0, 0.5)';
  ctx.beginPath();
  ctx.roundRect(meterX, meterTop, BAR_WIDTH, METER_HEIGHT, 4);
  ctx.fill();

  // Calculate stereo channel dimensions
  const channelWidth = (BAR_WIDTH - 4) / 2 - 1; // 2px gap in middle
  const rightX = meterX + 2 + channelWidth + 2;

  const effectiveLeftPeak = muted ? 0 : level.leftPeak;
  const effectiveRightPeak = muted ? 0 : level.rightPeak;
  const effectiveLeftRms = muted ? 0 : level.leftRms;
  const effectiveRightRms = muted ? 0 : level.rightRms;

  // 2. Left channel peak fill
  const leftPeakPosition = linearToPosition(effectiveLeftPeak);
  const leftPeakHeight = (leftPeakPosition / 100) * METER_HEIGHT;
  if (leftPeakHeight > 0) {
    ctx.fillStyle = registered.gradient!;
    ctx.beginPath();
    ctx.roundRect(meterX + 2, meterBottom - leftPeakHeight, channelWidth, leftPeakHeight, [0, 0, 2, 2]);
    ctx.fill();
  }

  // 3. Right channel peak fill
  const rightPeakPosition = linearToPosition(effectiveRightPeak);
  const rightPeakHeight = (rightPeakPosition / 100) * METER_HEIGHT;
  if (rightPeakHeight > 0) {
    ctx.fillStyle = registered.gradient!;
    ctx.beginPath();
    ctx.roundRect(rightX, meterBottom - rightPeakHeight, channelWidth, rightPeakHeight, [0, 0, 2, 2]);
    ctx.fill();
  }

  // 4. RMS lines on top of peak fill
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

  // 5. L/R labels at top
  ctx.font = '6px system-ui, sans-serif';
  ctx.textAlign = 'center';
  ctx.fillStyle = 'rgba(255, 255, 255, 0.7)';
  ctx.fillText('L', meterX + 2 + channelWidth / 2, meterTop + 8);
  ctx.fillText('R', rightX + channelWidth / 2, meterTop + 8);

  // 6. Stereo separator line
  const separatorX = meterX + 2 + channelWidth + 1;
  ctx.fillStyle = 'rgba(255, 255, 255, 0.5)';
  ctx.fillRect(separatorX - 0.5, meterTop, 1, METER_HEIGHT);

  // 7. dB scale markers (every 5dB)
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

  // 8. Reference level markers (-20dB and -9dB)
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

  // 8.5. Threshold markers for active filters (noise gate, compressor, expander)
  const activeThresholdFilters = thresholdFilters.filter(f => f.enabled !== false && f.threshold !== undefined);
  for (const filter of activeThresholdFilters) {
    const threshold = filter.threshold ?? -40;
    const position = dbToPosition(threshold);
    const y = dbToY(position);
    const color = THRESHOLD_COLORS[filter.type] || '#888888';

    ctx.strokeStyle = color;
    ctx.lineWidth = 2;
    ctx.setLineDash([4, 3]);
    ctx.beginPath();
    ctx.moveTo(meterX, y);
    ctx.lineTo(meterX + BAR_WIDTH, y);
    ctx.stroke();
    ctx.setLineDash([]);
  }

  // 9. Peak hold line - color coded based on level
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

  // 10. Volume arrow (filled when dragging, stroked outline when not)
  const volumeY = meterBottom - volume * METER_HEIGHT;
  const arrowX = meterX + BAR_WIDTH;

  ctx.beginPath();
  ctx.moveTo(arrowX, volumeY);
  ctx.lineTo(arrowX + ARROW_WIDTH, volumeY - ARROW_HEIGHT / 2);
  ctx.lineTo(arrowX + ARROW_WIDTH, volumeY + ARROW_HEIGHT / 2);
  ctx.closePath();

  if (isDragging) {
    // Filled style when dragging (primary color)
    ctx.fillStyle = '#7C3AED';
    ctx.fill();
  } else {
    // Stroked outline style (default)
    ctx.strokeStyle = muted ? 'rgba(107, 114, 128, 0.9)' : 'rgba(255, 255, 255, 0.9)';
    ctx.lineWidth = 2;
    ctx.stroke();
  }

  // 11. Muted overlay
  if (muted) {
    ctx.fillStyle = 'rgba(0, 0, 0, 0.4)';
    ctx.beginPath();
    ctx.roundRect(meterX, meterTop, BAR_WIDTH, METER_HEIGHT, 4);
    ctx.fill();
  }

  // 12. Clipping indicator at top
  if (level.clipping && !muted) {
    ctx.fillStyle = '#ef4444';
    ctx.beginPath();
    ctx.roundRect(meterX, meterTop, BAR_WIDTH, 4, [4, 4, 0, 0]);
    ctx.fill();
  }
}

/**
 * Render loop using setInterval instead of requestAnimationFrame.
 *
 * CRITICAL: Web Workers with requestAnimationFrame are STILL throttled/paused
 * when the browser tab is in the background. However, setInterval in Web Workers
 * continues running even when the tab is not focused.
 *
 * For a live streaming app, this is essential - audio meters must update
 * continuously regardless of whether the user is looking at the app.
 *
 * 30fps (33ms interval) provides smooth VU meter animation while being efficient.
 * This matches OBS's visual update rate for audio meters.
 */
const RENDER_INTERVAL_MS = 33; // ~30fps - smooth enough for VU meters, efficient
let intervalId: ReturnType<typeof setInterval> | null = null;

function renderLoop(): void {
  // Draw all registered canvases
  for (const registered of canvases.values()) {
    drawMeter(registered);
  }
}

function startRenderLoop(): void {
  if (intervalId !== null) return;
  // Use setInterval instead of requestAnimationFrame for background tab support
  intervalId = setInterval(renderLoop, RENDER_INTERVAL_MS);
  // Also run immediately
  renderLoop();
}

function stopRenderLoop(): void {
  if (intervalId !== null) {
    clearInterval(intervalId);
    intervalId = null;
  }
}

// Message handler
self.onmessage = (e: MessageEvent) => {
  const { type, id, data } = e.data;

  switch (type) {
    case 'init': {
      // Initialize with optional SharedArrayBuffer
      if (data.sharedBuffer instanceof SharedArrayBuffer) {
        const buffer = data.sharedBuffer as SharedArrayBuffer;
        sharedBuffer = buffer;
        sharedView = new Float32Array(buffer);
      }
      break;
    }

    case 'registerCanvas': {
      const { canvas, trackId, config } = data;
      if (!(canvas instanceof OffscreenCanvas)) {
        console.error('[AudioMeterWorker] Invalid canvas');
        return;
      }

      const ctx = canvas.getContext('2d', { alpha: false });
      if (!ctx) {
        console.error('[AudioMeterWorker] Failed to get 2D context');
        return;
      }

      // Set up canvas scaling for DPR
      const dpr = data.dpr || 1;
      canvas.width = TOTAL_WIDTH * dpr;
      canvas.height = TOTAL_HEIGHT * dpr;
      ctx.scale(dpr, dpr);

      canvases.set(id, {
        canvas,
        ctx,
        trackId,
        config: config || { volume: 1, muted: false },
        dpr,
        gradient: null,
        lastLevel: null,
      });

      // Start render loop if first canvas
      if (canvases.size === 1) {
        startRenderLoop();
      }
      break;
    }

    case 'unregisterCanvas': {
      canvases.delete(id);
      if (canvases.size === 0) {
        stopRenderLoop();
      }
      break;
    }

    case 'updateConfig': {
      const registered = canvases.get(id);
      if (registered && data.config) {
        registered.config = { ...registered.config, ...data.config };
      }
      break;
    }

    case 'audioData': {
      // Process raw WebSocket message
      if (typeof data === 'string') {
        processAudioData(data);
      }
      break;
    }
  }
};

// Signal that worker is ready
self.postMessage({ type: 'ready' });
