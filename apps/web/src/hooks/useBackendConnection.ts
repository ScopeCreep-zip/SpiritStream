import { useEffect } from 'react';
import { initBackendConnection, backendMode } from '@/lib/backend';

/**
 * Hook that initializes the backend connection on mount.
 * In HTTP mode, this eagerly establishes the WebSocket connection
 * to enable accurate connection status tracking.
 */
export function useBackendConnection() {
  useEffect(() => {
    if (backendMode === 'http') {
      initBackendConnection();
    }
  }, []);
}
