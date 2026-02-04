import { useEffect } from 'react';
import { initBackendConnection, backendMode } from '@/lib/backend';
import { preWarmGo2rtc } from '@/lib/go2rtcStatus';

/**
 * Hook that initializes the backend connection on mount.
 * In HTTP mode, this eagerly establishes the WebSocket connection
 * to enable accurate connection status tracking.
 *
 * Also pre-warms the go2rtc availability cache in the background
 * to reduce latency when WebRTC connections are started later.
 */
export function useBackendConnection() {
  useEffect(() => {
    if (backendMode === 'http') {
      initBackendConnection();
    }
    // Pre-warm go2rtc availability cache in background
    // This runs whether we're in HTTP or Tauri mode
    preWarmGo2rtc();
  }, []);
}
