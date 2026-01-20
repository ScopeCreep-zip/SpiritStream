import { getBackendWsUrl } from './env';
import { useConnectionStore } from '@/stores/connectionStore';

type Handler<T> = (payload: T) => void;

type AnyHandler = Handler<unknown>;

const handlers = new Map<string, Set<AnyHandler>>();
let socket: WebSocket | null = null;
let openPromise: Promise<void> | null = null;
let reconnectTimer: number | null = null;

// Track if we were previously connected (for showing reconnection toasts)
let wasConnected = false;

// When true, keep the connection alive even without handlers (for status tracking)
let keepAlive = false;

function notifyConnected() {
  const store = useConnectionStore.getState();
  const wasReconnecting = store.reconnectAttempts > 0;
  store.setConnected();
  wasConnected = true;

  // Emit custom event for reconnection notifications
  if (wasReconnecting) {
    window.dispatchEvent(new CustomEvent('backend:reconnected'));
  } else {
    window.dispatchEvent(new CustomEvent('backend:connected'));
  }
}

function notifyDisconnected(error?: string) {
  const store = useConnectionStore.getState();
  store.setDisconnected(error);

  if (wasConnected) {
    window.dispatchEvent(new CustomEvent('backend:disconnected', { detail: { error } }));
  }
}

function notifyConnecting() {
  const store = useConnectionStore.getState();
  store.setConnecting();
  store.incrementReconnectAttempts();
}

function notifyAuthRequired() {
  window.dispatchEvent(new CustomEvent('backend:auth-required'));
}

function ensureSocket(): Promise<void> {
  if (socket && socket.readyState === WebSocket.OPEN) {
    return Promise.resolve();
  }

  if (openPromise) {
    return openPromise;
  }

  notifyConnecting();

  // WebSocket URL without token - cookies are sent automatically for same-origin requests
  const wsUrl = getBackendWsUrl();
  console.debug(`[httpEvents] Connecting to WebSocket: ${wsUrl}`);

  openPromise = new Promise((resolve) => {
    socket = new WebSocket(wsUrl);

    socket.addEventListener('open', () => {
      openPromise = null;
      notifyConnected();
      resolve();
    });

    socket.addEventListener('message', (event) => {
      if (!event.data) return;
      try {
        const parsed = JSON.parse(event.data as string) as {
          event?: string;
          payload?: unknown;
        };
        if (!parsed.event) return;
        const listeners = handlers.get(parsed.event);
        if (!listeners || listeners.size === 0) return;
        for (const handler of listeners) {
          handler(parsed.payload);
        }
      } catch (error) {
        console.warn('Failed to parse backend event:', error);
      }
    });

    socket.addEventListener('close', (event) => {
      socket = null;
      openPromise = null;

      // Check if this is an auth failure (401 Unauthorized returns code 1008)
      if (event.code === 1008 || event.reason === 'Unauthorized') {
        notifyAuthRequired();
        notifyDisconnected('Authentication required');
        return;
      }

      notifyDisconnected();
      if (handlers.size > 0 || keepAlive) {
        scheduleReconnect();
      }
    });

    socket.addEventListener('error', (event) => {
      console.error('[httpEvents] WebSocket error:', event);
      if (socket && socket.readyState === WebSocket.OPEN) {
        return;
      }
      socket = null;
      openPromise = null;
      notifyDisconnected('Connection error');
      if (handlers.size > 0 || keepAlive) {
        scheduleReconnect();
      }
    });
  });

  return openPromise;
}

function scheduleReconnect() {
  if (reconnectTimer) return;
  reconnectTimer = window.setTimeout(() => {
    reconnectTimer = null;
    ensureSocket().catch(() => {
      scheduleReconnect();
    });
  }, 1000);
}

/**
 * Initialize the WebSocket connection proactively.
 * Call this early in app startup to establish connection status.
 * The connection will be kept alive and auto-reconnect even without handlers.
 */
export function initConnection(): void {
  keepAlive = true;
  ensureSocket().catch((err) => {
    console.error('[httpEvents] Failed to initialize connection:', err);
  });
}

/**
 * Disconnect and clear the WebSocket connection.
 * Call this when logging out to clean up.
 */
export function disconnectSocket(): void {
  keepAlive = false;
  if (reconnectTimer) {
    window.clearTimeout(reconnectTimer);
    reconnectTimer = null;
  }
  if (socket) {
    socket.close();
    socket = null;
  }
  openPromise = null;
}

export const events = {
  on: async <T>(eventName: string, handler: Handler<T>): Promise<() => void> => {
    const set = handlers.get(eventName) ?? new Set<AnyHandler>();
    set.add(handler as AnyHandler);
    handlers.set(eventName, set);

    await ensureSocket();

    return () => {
      const listeners = handlers.get(eventName);
      if (listeners) {
        listeners.delete(handler as AnyHandler);
        if (listeners.size === 0) {
          handlers.delete(eventName);
        }
      }

      if (handlers.size === 0 && !keepAlive && socket) {
        socket.close();
        socket = null;
      }
    };
  },
};
