/**
 * go2rtc Availability Cache
 *
 * With lazy go2rtc startup, go2rtc is not available at app start.
 * It starts on first WebRTC request. This module:
 * - Listens for the server's `go2rtc_status` WebSocket event
 * - Falls back to a single API check (no retry loop)
 * - Caches the result once known
 */

import { api } from '@/lib/backend';
import { events } from '@/lib/backend/httpEvents';

/** Cached availability state: null = not checked, true/false = result */
let go2rtcAvailable: boolean | null = null;

/** Promise for in-flight availability check to prevent duplicate requests */
let checkPromise: Promise<boolean> | null = null;

// Listen for server-pushed go2rtc_status event (fires after lazy start)
if (typeof window !== 'undefined') {
  events.on<{ available?: boolean }>('go2rtc_status', (payload) => {
    if (payload?.available) {
      go2rtcAvailable = true;
      checkPromise = null;
    }
  }).catch(() => {
    // WebSocket not ready yet — will get event when it connects
  });
}

/**
 * Check if go2rtc is available, with caching.
 *
 * With lazy startup, go2rtc won't be available until the first WebRTC
 * stream is requested. This does a single check (no retries) and
 * relies on the `go2rtc_status` event for lazy-start notification.
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

  // Single check — no retry loop. go2rtc starts lazily on first use.
  checkPromise = (async () => {
    try {
      const available = await api.webrtc.isAvailable();
      if (available) {
        go2rtcAvailable = true;
        return true;
      }
    } catch {
      // go2rtc not started yet — expected with lazy startup
    }

    // Not available yet — will be updated by go2rtc_status event
    go2rtcAvailable = false;
    return false;
  })();

  return checkPromise;
}

/**
 * Pre-warm the go2rtc availability cache.
 *
 * With lazy startup this is essentially a quick status check.
 * The real availability notification comes via WebSocket event.
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
