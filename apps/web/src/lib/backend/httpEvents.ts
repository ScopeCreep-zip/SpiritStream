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

// Reconnection limits to prevent infinite loops when backend is unavailable
const MAX_RECONNECT_ATTEMPTS = 30;
const INITIAL_RECONNECT_DELAY = 1000; // 1 second
const MAX_RECONNECT_DELAY = 30000; // 30 seconds
let reconnectCount = 0;

// When true, keep the connection alive even without handlers (for status tracking)
let keepAlive = false;

function notifyConnected() {
  const store = useConnectionStore.getState();
  const wasReconnecting = store.reconnectAttempts > 0;
  store.setConnected();
  wasConnected = true;

  // Reset reconnection counter on successful connection
  reconnectCount = 0;

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

  // Build WebSocket URL, adding token for cross-origin connections
  // (cookies may not be sent due to CORS restrictions)
  let wsUrl = getBackendWsUrl();

  // Check if this is a cross-origin connection
  if (typeof window !== 'undefined') {
    const wsHost = new URL(wsUrl.replace('ws://', 'http://').replace('wss://', 'https://')).host;
    const isCrossOrigin = wsHost !== window.location.host;

    if (isCrossOrigin) {
      // For cross-origin connections, include token in query params
      // Backend supports both cookie auth and token query param
      const token = window.localStorage.getItem('spiritstream-auth-token');
      if (token) {
        const separator = wsUrl.includes('?') ? '&' : '?';
        wsUrl = `${wsUrl}${separator}token=${encodeURIComponent(token)}`;
        console.debug('[httpEvents] Cross-origin connection, added token to URL');
      }
    }
  }

  console.debug(`[httpEvents] Connecting to WebSocket: ${wsUrl.replace(/token=[^&]+/, 'token=***')}`);

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

  // Check if we've exceeded max reconnection attempts
  if (reconnectCount >= MAX_RECONNECT_ATTEMPTS) {
    console.error(
      `[httpEvents] Max reconnection attempts (${MAX_RECONNECT_ATTEMPTS}) reached. ` +
        'Please refresh the page or check backend server status.'
    );
    notifyDisconnected('Connection lost. Please refresh the page.');
    return;
  }

  reconnectCount++;

  // Exponential backoff: 1s, 1.5s, 2.25s, ... up to 30s max
  const delay = Math.min(
    INITIAL_RECONNECT_DELAY * Math.pow(1.5, reconnectCount - 1),
    MAX_RECONNECT_DELAY
  );

  console.debug(
    `[httpEvents] Scheduling reconnect attempt ${reconnectCount}/${MAX_RECONNECT_ATTEMPTS} in ${Math.round(delay)}ms`
  );

  reconnectTimer = window.setTimeout(() => {
    reconnectTimer = null;
    ensureSocket().catch(() => {
      scheduleReconnect();
    });
  }, delay);
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
  reconnectCount = 0; // Reset counter for fresh start on next connection
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
