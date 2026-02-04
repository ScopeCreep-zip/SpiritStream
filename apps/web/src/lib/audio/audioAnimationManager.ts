/**
 * Audio Animation Manager
 *
 * Single centralized RAF loop that handles ALL audio meter updates.
 * Instead of each channel strip running its own RAF loop + intervals,
 * components register their render callbacks with this manager.
 *
 * Benefits:
 * - 1 RAF loop instead of N (one per channel)
 * - No setInterval overhead
 * - Coordinated frame timing
 * - Reduced main thread contention
 */

type RenderCallback = (timestamp: number) => void;

interface RegisteredRenderer {
  callback: RenderCallback;
  lastPeakDbUpdate: number;
  lastClipCheck: number;
}

// Singleton state
const renderers = new Map<string, RegisteredRenderer>();
let rafId: number | null = null;
let isRunning = false;

// Timing constants
const PEAK_DB_UPDATE_INTERVAL = 100; // 10Hz for peak dB display updates
const CLIP_CHECK_INTERVAL = 100; // 10Hz for clipping checks

/**
 * Main animation loop - runs at display refresh rate
 * but only redraws when audio data has changed
 */
function animationLoop(timestamp: number): void {
  if (!isRunning) return;

  // Call all registered renderers
  for (const renderer of renderers.values()) {
    renderer.callback(timestamp);
  }

  rafId = requestAnimationFrame(animationLoop);
}

/**
 * Register a meter renderer.
 * Returns an unregister function.
 */
export function registerMeterRenderer(
  id: string,
  callback: RenderCallback
): () => void {
  renderers.set(id, {
    callback,
    lastPeakDbUpdate: 0,
    lastClipCheck: 0,
  });

  // Start the loop if not running
  if (!isRunning) {
    isRunning = true;
    rafId = requestAnimationFrame(animationLoop);
  }

  // Return unregister function
  return () => {
    renderers.delete(id);

    // Stop the loop if no renderers left
    if (renderers.size === 0 && rafId !== null) {
      isRunning = false;
      cancelAnimationFrame(rafId);
      rafId = null;
    }
  };
}

/**
 * Check if enough time has passed for a peak dB update
 */
export function shouldUpdatePeakDb(id: string, timestamp: number): boolean {
  const renderer = renderers.get(id);
  if (!renderer) return false;

  if (timestamp - renderer.lastPeakDbUpdate >= PEAK_DB_UPDATE_INTERVAL) {
    renderer.lastPeakDbUpdate = timestamp;
    return true;
  }
  return false;
}

/**
 * Check if enough time has passed for a clip check
 */
export function shouldCheckClipping(id: string, timestamp: number): boolean {
  const renderer = renderers.get(id);
  if (!renderer) return false;

  if (timestamp - renderer.lastClipCheck >= CLIP_CHECK_INTERVAL) {
    renderer.lastClipCheck = timestamp;
    return true;
  }
  return false;
}

/**
 * Get the number of active renderers (for debugging)
 */
export function getActiveRendererCount(): number {
  return renderers.size;
}
