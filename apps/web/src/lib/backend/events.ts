import { backendMode } from './env';
import { events as tauriEvents } from './tauriEvents';
import { events as httpEvents, initConnection as initHttpConnection } from './httpEvents';

export const events = backendMode === 'tauri' ? tauriEvents : httpEvents;

/**
 * Initialize backend connection. In HTTP mode, this eagerly establishes
 * the WebSocket connection for status tracking.
 */
export function initBackendConnection(): void {
  if (backendMode === 'http') {
    initHttpConnection();
  }
}
