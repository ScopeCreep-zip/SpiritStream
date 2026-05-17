import { useEffect } from 'react';
import { initConnection } from '@spiritstream/api-client';

/**
 * Hook that initializes the backend connection on mount.
 * Tauri 2 webview, Docker, and browser all talk HTTP to the same surface;
 * the WebSocket sits behind /api/v1/events and provides connection-status
 * tracking via window CustomEvents the connection store listens for.
 */
export function useBackendConnection() {
  useEffect(() => {
    initConnection();
  }, []);
}
