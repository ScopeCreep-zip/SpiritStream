/**
 * go2rtc Availability Cache
 *
 * Provides a cached check for go2rtc availability to avoid repeated
 * startup delays in WebRTC connection code. The availability status
 * is cached after the first successful check.
 *
 * Usage:
 * - Call preWarmGo2rtc() early during app initialization
 * - Use isGo2rtcAvailable() when starting WebRTC connections
 *
 * Performance optimizations:
 * - Reduced retry delay from 500ms to 200ms for faster cold start
 * - Pre-warms RTCPeerConnection pool when go2rtc becomes available
 */

import { api } from '@/lib/backend';

/** Cached availability state: null = not checked, true/false = result */
let go2rtcAvailable: boolean | null = null;

/** Promise for in-flight availability check to prevent duplicate requests */
let checkPromise: Promise<boolean> | null = null;

/** Maximum retry attempts for availability check */
const MAX_RETRIES = 15;

/** Delay between retries in milliseconds - reduced from 500ms to 200ms for faster cold start */
const RETRY_DELAY_MS = 200;

/**
 * Check if go2rtc is available, with caching.
 *
 * On first call, performs retries with 200ms delays until go2rtc responds
 * or max retries are exhausted (3s total vs previous 5s). Subsequent calls
 * return cached result immediately.
 *
 * @returns Promise that resolves to true if go2rtc is available, false otherwise
 */
export async function isGo2rtcAvailable(): Promise<boolean> {
  // Return cached result if available
  if (go2rtcAvailable !== null) {
    return go2rtcAvailable;
  }

  // Return existing promise if check is in progress
  if (checkPromise) {
    return checkPromise;
  }

  // Start new availability check with retries
  checkPromise = (async () => {
    for (let i = 0; i < MAX_RETRIES; i++) {
      try {
        const available = await api.webrtc.isAvailable();
        if (available) {
          go2rtcAvailable = true;
          // Pre-warm WebRTC connection pool now that go2rtc is available
          // This reduces first-connection latency by having connections ready
          try {
            const { preWarmConnectionPool } = await import('@/stores/webrtcConnectionStore');
            preWarmConnectionPool(2);
          } catch {
            // Ignore if module not available
          }
          return true;
        }
      } catch {
        // Ignore errors during startup check
      }

      // Wait before retry (skip wait on last attempt)
      if (i < MAX_RETRIES - 1) {
        await new Promise((resolve) => setTimeout(resolve, RETRY_DELAY_MS));
      }
    }

    go2rtcAvailable = false;
    return false;
  })();

  return checkPromise;
}

/**
 * Pre-warm the go2rtc availability cache.
 *
 * Call this early during app initialization (e.g., after WebSocket connects)
 * to start the availability check in background. This reduces latency when
 * WebRTC connections are started later.
 *
 * Fire-and-forget: does not wait for result or throw errors.
 */
export function preWarmGo2rtc(): void {
  isGo2rtcAvailable().catch(() => {
    // Ignore errors during pre-warming
  });
}

/**
 * Reset the cached availability state.
 *
 * Use this if go2rtc is restarted and availability needs to be re-checked.
 */
export function resetGo2rtcCache(): void {
  go2rtcAvailable = null;
  checkPromise = null;
}

/**
 * Get the current cached availability status without performing a check.
 *
 * @returns true if available, false if not available, null if not yet checked
 */
export function getGo2rtcCachedStatus(): boolean | null {
  return go2rtcAvailable;
}
