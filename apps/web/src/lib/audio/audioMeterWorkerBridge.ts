/**
 * Audio Meter Worker Bridge
 *
 * Promise-based wrapper for the audio meter Web Worker.
 * Inspired by Comlink's approach - all operations return Promises and are
 * automatically queued if the worker isn't ready yet.
 *
 * KEY PATTERN: Components don't need to check if worker is ready.
 * Just call registerMeterCanvas() - it returns a Promise that resolves
 * when the canvas is actually transferred to the worker.
 *
 * For React Suspense integration, use the exported `workerReady` promise:
 *   const ready = use(workerReady); // React 19
 *   // or wrap in Suspense boundary
 *
 * References:
 * - Comlink: https://github.com/GoogleChromeLabs/comlink
 * - React 19 use() hook: https://react.dev/reference/react/use
 */

import AudioMeterWorkerUrl from './audioMeterWorker.ts?worker&url';

// Worker instance (singleton)
let worker: Worker | null = null;
let isReady = false;

// Promise that resolves when worker is ready - for Suspense integration
let workerReadyResolve: () => void;
export const workerReady: Promise<void> = new Promise((resolve) => {
  workerReadyResolve = resolve;
});

// Pending operations queue - executed when worker becomes ready
interface PendingOperation {
  message: unknown;
  transferables?: Transferable[];
  resolve: (value: string) => void;
  reject: (error: Error) => void;
}
const pendingOperations: PendingOperation[] = [];

// Track canvases that have been transferred (can only transfer once)
const transferredCanvases = new WeakSet<HTMLCanvasElement>();
const canvasIds = new WeakMap<HTMLCanvasElement, string>();

// Per-canvas ready callbacks
const canvasReadyCallbacks = new Map<string, Array<(id: string) => void>>();

// SharedArrayBuffer for zero-copy level reads (optional)
let sharedBuffer: SharedArrayBuffer | null = null;
let sharedView: Float32Array | null = null;

// Canvas ID counter
let nextCanvasId = 0;

// SharedArrayBuffer layout
const BUFFER_SIZE = 1 + 8 + 16 * 8;
const MASTER_OFFSET = 1;

/**
 * Initialize the audio meter worker.
 * Call this early in app startup. The workerReady promise resolves when done.
 */
export function initAudioMeterWorker(): void {
  if (worker) return;

  try {
    worker = new Worker(AudioMeterWorkerUrl, { type: 'module' });

    worker.onmessage = (e: MessageEvent) => {
      if (e.data.type === 'ready') {
        isReady = true;
        console.log('[AudioMeterWorker] Worker ready');

        // Try SharedArrayBuffer (requires COOP/COEP headers)
        try {
          if (typeof SharedArrayBuffer !== 'undefined') {
            sharedBuffer = new SharedArrayBuffer(BUFFER_SIZE * 4);
            sharedView = new Float32Array(sharedBuffer);
            worker?.postMessage({ type: 'init', data: { sharedBuffer } });
          }
        } catch {
          console.debug('[AudioMeterWorker] SharedArrayBuffer not available');
        }

        // Process all pending operations
        processPendingOperations();

        // Resolve the workerReady promise (for Suspense)
        workerReadyResolve();
      }
    };

    worker.onerror = (err) => {
      console.error('[AudioMeterWorker] Worker error:', err);
    };
  } catch (err) {
    console.error('[AudioMeterWorker] Failed to create worker:', err);
  }
}

/**
 * Process all queued operations now that worker is ready.
 */
function processPendingOperations(): void {
  if (!worker || !isReady) return;

  console.log(`[AudioMeterWorker] Processing ${pendingOperations.length} pending operations`);

  while (pendingOperations.length > 0) {
    const op = pendingOperations.shift()!;
    try {
      if (op.transferables) {
        worker.postMessage(op.message, op.transferables);
      } else {
        worker.postMessage(op.message);
      }
      // For canvas registration, the ID was already assigned
      op.resolve((op.message as { id?: string }).id || '');
    } catch (err) {
      op.reject(err instanceof Error ? err : new Error(String(err)));
    }
  }
}

/**
 * Send a message to the worker, queueing if not ready.
 * Returns a Promise that resolves when the message is sent.
 */
function postMessageAsync(
  message: unknown,
  transferables?: Transferable[]
): Promise<string> {
  return new Promise((resolve, reject) => {
    if (isReady && worker) {
      // Worker ready - send immediately
      try {
        if (transferables) {
          worker.postMessage(message, transferables);
        } else {
          worker.postMessage(message);
        }
        resolve((message as { id?: string }).id || '');
      } catch (err) {
        reject(err);
      }
    } else {
      // Queue for later
      pendingOperations.push({ message, transferables, resolve, reject });
    }
  });
}

/**
 * Terminate the worker and clean up.
 */
export function terminateAudioMeterWorker(): void {
  if (worker) {
    worker.terminate();
    worker = null;
    isReady = false;
    sharedBuffer = null;
    sharedView = null;
    pendingOperations.length = 0;
    canvasReadyCallbacks.clear();
  }
}

/**
 * Check if the worker is ready (for non-Promise-based checks).
 */
export function isWorkerReady(): boolean {
  return isReady;
}

/**
 * Check if SharedArrayBuffer is available.
 */
export function hasSharedMemory(): boolean {
  return sharedView !== null;
}

/** Config for meter rendering */
export interface MeterConfig {
  volume: number;
  muted: boolean;
  isDragging?: boolean;
  thresholdFilters?: Array<{
    type: string;
    threshold?: number;
    enabled?: boolean;
  }>;
}

/**
 * Register a canvas element for meter rendering.
 * Returns a Promise that resolves with the canvas ID when the canvas
 * is actually transferred to the worker (not just queued).
 *
 * This is the KEY API - components just await this Promise.
 * No need to check worker readiness or handle fallback.
 *
 * @example
 * const canvasId = await registerMeterCanvas(canvas, trackId, config);
 * // Canvas is now being rendered by worker
 */
export async function registerMeterCanvas(
  canvas: HTMLCanvasElement,
  trackId: string | null,
  config: MeterConfig
): Promise<string> {
  // Check if already registered (React StrictMode)
  if (transferredCanvases.has(canvas)) {
    const existingId = canvasIds.get(canvas);
    if (existingId) {
      console.debug(`[AudioMeterWorker] Canvas already registered as ${existingId}`);
      return existingId;
    }
    throw new Error('Canvas was transferred but ID lost');
  }

  const id = `meter-${nextCanvasId++}`;
  const dpr = window.devicePixelRatio || 1;

  // Transfer canvas to offscreen
  const offscreen = canvas.transferControlToOffscreen();
  transferredCanvases.add(canvas);
  canvasIds.set(canvas, id);

  const message = {
    type: 'registerCanvas',
    id,
    data: { canvas: offscreen, trackId, config, dpr },
  };

  // Send to worker (queued if not ready)
  await postMessageAsync(message, [offscreen]);

  console.debug(`[AudioMeterWorker] Registered canvas ${id} for track ${trackId ?? 'master'}`);

  // Notify any listeners
  const callbacks = canvasReadyCallbacks.get(id);
  if (callbacks) {
    callbacks.forEach(cb => cb(id));
    canvasReadyCallbacks.delete(id);
  }

  return id;
}

/**
 * Check if a canvas is registered with the worker.
 */
export function isCanvasRegistered(canvas: HTMLCanvasElement): boolean {
  return transferredCanvases.has(canvas);
}

/**
 * Check if a canvas has been transferred (same as isCanvasRegistered now).
 */
export function isCanvasTransferred(canvas: HTMLCanvasElement): boolean {
  return transferredCanvases.has(canvas);
}

/**
 * Get the ID for a registered canvas.
 */
export function getCanvasId(canvas: HTMLCanvasElement): string | null {
  return canvasIds.get(canvas) ?? null;
}

/**
 * Unregister a canvas.
 */
export function unregisterMeterCanvas(id: string): void {
  if (isReady && worker) {
    worker.postMessage({ type: 'unregisterCanvas', id });
  }
}

/**
 * Update config for a registered canvas.
 */
export function updateMeterConfig(
  id: string,
  config: Partial<MeterConfig>
): void {
  if (isReady && worker) {
    worker.postMessage({ type: 'updateConfig', id, data: { config } });
  }
}

/**
 * Forward raw WebSocket audio data to the worker.
 */
export function forwardAudioData(rawMessage: string): void {
  if (isReady && worker) {
    worker.postMessage({ type: 'audioData', data: rawMessage });
  }
}

/**
 * Read master level from SharedArrayBuffer (zero-copy).
 */
export function getMasterLevelFromShared(): {
  rms: number;
  peak: number;
  leftRms: number;
  leftPeak: number;
  rightRms: number;
  rightPeak: number;
  peakDb: number;
  clipping: boolean;
} | null {
  if (!sharedView) return null;

  return {
    rms: sharedView[MASTER_OFFSET + 0],
    peak: sharedView[MASTER_OFFSET + 1],
    leftRms: sharedView[MASTER_OFFSET + 2],
    leftPeak: sharedView[MASTER_OFFSET + 3],
    rightRms: sharedView[MASTER_OFFSET + 4],
    rightPeak: sharedView[MASTER_OFFSET + 5],
    peakDb: sharedView[MASTER_OFFSET + 6],
    clipping: sharedView[MASTER_OFFSET + 7] !== 0,
  };
}

/**
 * Get the current data version from SharedArrayBuffer.
 */
export function getSharedVersion(): number {
  return sharedView ? sharedView[0] : 0;
}
