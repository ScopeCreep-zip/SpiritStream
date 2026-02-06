/**
 * useVideoPreviewWorker Hook
 *
 * Manages the video preview worker for off-main-thread rendering.
 * Provides functions to register/unregister canvases and send frames.
 *
 * PERFORMANCE:
 * - OffscreenCanvas rendering happens in a separate thread
 * - ImageBitmap transfers are zero-copy (no data duplication)
 * - Main thread only sends messages, never blocks
 *
 * Usage:
 * 1. Call useVideoPreviewWorker() to get the singleton worker instance
 * 2. Use registerCanvas() to transfer an OffscreenCanvas to the worker
 * 3. Call sendFrame() with ImageBitmap from video.requestVideoFrameCallback
 * 4. Call unregisterCanvas() on cleanup
 */

import { useEffect, useCallback, useRef } from 'react';
import type { VideoPreviewMessage, VideoPreviewResponse } from '@/workers/videoPreviewWorker';

// Singleton worker instance
let workerInstance: Worker | null = null;
let workerReady = false;
let workerReadyPromise: Promise<void> | null = null;
const pendingCallbacks = new Map<string, (response: VideoPreviewResponse) => void>();

/**
 * Initialize the worker singleton
 */
function getWorker(): Worker {
  if (!workerInstance) {
    // Create worker with module type for ES module support
    workerInstance = new Worker(
      new URL('@/workers/videoPreviewWorker.ts', import.meta.url),
      { type: 'module' }
    );

    // Handle responses from worker
    workerInstance.onmessage = (event: MessageEvent<VideoPreviewResponse>) => {
      const response = event.data;

      if (response.type === 'ready') {
        workerReady = true;
        return;
      }

      // Handle canvas-specific callbacks
      if (response.canvasId && pendingCallbacks.has(response.canvasId)) {
        const callback = pendingCallbacks.get(response.canvasId)!;
        pendingCallbacks.delete(response.canvasId);
        callback(response);
      }
    };

    workerInstance.onerror = (error) => {
      console.error('[VideoPreviewWorker] Worker error:', error);
    };

    // Initialize worker
    workerReadyPromise = new Promise<void>((resolve) => {
      const checkReady = () => {
        if (workerReady) {
          resolve();
        } else {
          setTimeout(checkReady, 10);
        }
      };

      // Send init message
      workerInstance!.postMessage({ type: 'init' } as VideoPreviewMessage);
      checkReady();
    });
  }

  return workerInstance;
}

/**
 * Wait for worker to be ready
 */
async function ensureWorkerReady(): Promise<Worker> {
  const worker = getWorker();
  if (workerReadyPromise) {
    await workerReadyPromise;
  }
  return worker;
}

/**
 * Register an OffscreenCanvas with the worker
 * Transfers ownership to the worker (canvas becomes unusable in main thread)
 */
export async function registerCanvas(
  canvasId: string,
  canvas: HTMLCanvasElement
): Promise<boolean> {
  const worker = await ensureWorkerReady();

  // Create OffscreenCanvas and transfer ownership
  // Note: This makes the original canvas unusable for direct drawing
  const offscreen = canvas.transferControlToOffscreen();

  return new Promise<boolean>((resolve) => {
    // Set up callback for this canvas
    pendingCallbacks.set(canvasId, (response) => {
      resolve(response.type === 'registered');
    });

    // Send message with transferable canvas
    const message: VideoPreviewMessage = {
      type: 'registerCanvas',
      canvasId,
      canvas: offscreen,
    };

    worker.postMessage(message, [offscreen]);

    // Timeout after 5 seconds
    setTimeout(() => {
      if (pendingCallbacks.has(canvasId)) {
        pendingCallbacks.delete(canvasId);
        resolve(false);
      }
    }, 5000);
  });
}

/**
 * Unregister a canvas from the worker
 */
export async function unregisterCanvas(canvasId: string): Promise<void> {
  const worker = await ensureWorkerReady();

  const message: VideoPreviewMessage = {
    type: 'unregisterCanvas',
    canvasId,
  };

  worker.postMessage(message);
}

/**
 * Send a video frame to be rendered
 * Uses transferable ImageBitmap for zero-copy transfer
 */
export function sendFrame(canvasId: string, frame: ImageBitmap): void {
  if (!workerInstance || !workerReady) {
    frame.close(); // Clean up if worker not ready
    return;
  }

  const message: VideoPreviewMessage = {
    type: 'frame',
    canvasId,
    frame,
  };

  // Transfer the ImageBitmap to the worker (zero-copy)
  workerInstance.postMessage(message, [frame]);
}

/**
 * Notify worker of canvas resize
 */
export function resizeCanvas(canvasId: string, width: number, height: number): void {
  if (!workerInstance || !workerReady) {
    return;
  }

  const message: VideoPreviewMessage = {
    type: 'resize',
    canvasId,
    width,
    height,
  };

  workerInstance.postMessage(message);
}

/**
 * Hook for components to use the video preview worker
 * Returns helper functions bound to a specific canvas ID
 */
export function useVideoPreviewWorker(canvasId: string) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const registeredRef = useRef(false);
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const rafIdRef = useRef<number | null>(null);

  /**
   * Register a canvas element for worker rendering
   */
  const register = useCallback(async (canvas: HTMLCanvasElement): Promise<boolean> => {
    if (registeredRef.current) {
      return true;
    }

    canvasRef.current = canvas;
    const success = await registerCanvas(canvasId, canvas);
    registeredRef.current = success;
    return success;
  }, [canvasId]);

  /**
   * Unregister the canvas
   */
  const unregister = useCallback(async () => {
    if (registeredRef.current) {
      await unregisterCanvas(canvasId);
      registeredRef.current = false;
    }
    canvasRef.current = null;
  }, [canvasId]);

  // Track the video frame callback ID separately from RAF
  const vfcIdRef = useRef<number | null>(null);
  const captureActiveRef = useRef(false);

  /**
   * Start capturing frames from a video element
   * Uses requestVideoFrameCallback for precise frame timing
   */
  const startCapture = useCallback((video: HTMLVideoElement) => {
    // Stop any existing capture first
    if (captureActiveRef.current) {
      return; // Already capturing
    }

    videoRef.current = video;
    captureActiveRef.current = true;

    // Use requestVideoFrameCallback if available (Chrome 83+)
    // This provides much better frame timing than requestAnimationFrame
    if ('requestVideoFrameCallback' in video) {
      const captureFrame = async () => {
        if (!captureActiveRef.current || !videoRef.current || !registeredRef.current) {
          return;
        }

        try {
          // Create ImageBitmap from current video frame
          // This is zero-copy on most browsers
          const bitmap = await createImageBitmap(videoRef.current);
          if (captureActiveRef.current) {
            sendFrame(canvasId, bitmap);
          } else {
            bitmap.close(); // Clean up if capture stopped
          }
        } catch {
          // Video might not be ready yet
        }

        // Request next frame only if still capturing
        if (captureActiveRef.current && videoRef.current && 'requestVideoFrameCallback' in videoRef.current) {
          vfcIdRef.current = videoRef.current.requestVideoFrameCallback(captureFrame);
        }
      };

      vfcIdRef.current = video.requestVideoFrameCallback(captureFrame);
    } else {
      // Fallback to requestAnimationFrame for older browsers
      const captureFrame = async () => {
        if (!captureActiveRef.current || !videoRef.current || !registeredRef.current) {
          return;
        }

        try {
          const bitmap = await createImageBitmap(videoRef.current);
          if (captureActiveRef.current) {
            sendFrame(canvasId, bitmap);
          } else {
            bitmap.close();
          }
        } catch {
          // Video might not be ready yet
        }

        if (captureActiveRef.current) {
          rafIdRef.current = requestAnimationFrame(captureFrame);
        }
      };

      rafIdRef.current = requestAnimationFrame(captureFrame);
    }
  }, [canvasId]);

  /**
   * Stop capturing frames
   */
  const stopCapture = useCallback(() => {
    captureActiveRef.current = false;

    // Cancel requestVideoFrameCallback
    if (vfcIdRef.current !== null && videoRef.current && 'cancelVideoFrameCallback' in videoRef.current) {
      (videoRef.current as HTMLVideoElement & { cancelVideoFrameCallback: (id: number) => void })
        .cancelVideoFrameCallback(vfcIdRef.current);
      vfcIdRef.current = null;
    }

    // Cancel requestAnimationFrame
    if (rafIdRef.current !== null) {
      cancelAnimationFrame(rafIdRef.current);
      rafIdRef.current = null;
    }

    videoRef.current = null;
  }, []);

  /**
   * Handle canvas resize
   */
  const resize = useCallback((width: number, height: number) => {
    resizeCanvas(canvasId, width, height);
  }, [canvasId]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      stopCapture();
      // Don't unregister on unmount - let the canvas be reused
    };
  }, [stopCapture]);

  return {
    register,
    unregister,
    startCapture,
    stopCapture,
    resize,
    isRegistered: () => registeredRef.current,
  };
}

/**
 * Check if OffscreenCanvas is supported
 */
export function isOffscreenCanvasSupported(): boolean {
  return typeof OffscreenCanvas !== 'undefined' &&
    typeof HTMLCanvasElement.prototype.transferControlToOffscreen === 'function';
}
