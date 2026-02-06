import { getBackendWsUrl } from './env';
import { useConnectionStore } from '@/stores/connectionStore';
import { forwardAudioData, isWorkerReady } from '@/lib/audio/audioMeterWorkerBridge';

type Handler<T> = (payload: T) => void;

type AnyHandler = Handler<unknown>;

const handlers = new Map<string, Set<AnyHandler>>();
let socket: WebSocket | null = null;
let openPromise: Promise<void> | null = null;
let reconnectTimer: number | null = null;

// Store listener references for cleanup to prevent memory leaks on reconnection
// Each reconnection previously added NEW listeners while old ones remained attached
let currentListeners: {
  open: (() => void) | null;
  message: ((e: MessageEvent) => void) | null;
  close: ((e: CloseEvent) => void) | null;
  error: (() => void) | null;
} = { open: null, message: null, close: null, error: null };

/**
 * Remove all event listeners from the current socket.
 * CRITICAL: Must be called before creating new listeners to prevent memory leaks.
 */
function cleanupSocketListeners(): void {
  if (socket) {
    if (currentListeners.open) socket.removeEventListener('open', currentListeners.open);
    if (currentListeners.message) socket.removeEventListener('message', currentListeners.message);
    if (currentListeners.close) socket.removeEventListener('close', currentListeners.close);
    if (currentListeners.error) socket.removeEventListener('error', currentListeners.error);
  }
  currentListeners = { open: null, message: null, close: null, error: null };
}

// Track if we were previously connected (for showing reconnection toasts)
let wasConnected = false;

// Reconnection limits to prevent infinite loops when backend is unavailable
const MAX_RECONNECT_ATTEMPTS = 30;
const INITIAL_RECONNECT_DELAY = 1000; // 1 second
const MAX_RECONNECT_DELAY = 30000; // 30 seconds
let reconnectCount = 0;

// When true, keep the connection alive even without handlers (for status tracking)
let keepAlive = false;

// WebSocket keepalive for background tab support
// Browsers may close idle WebSockets after ~5 minutes in background
const KEEPALIVE_INTERVAL_MS = 30000; // 30 seconds - send ping to keep connection alive
let keepaliveTimer: number | null = null;

/**
 * Start keepalive ping timer.
 * This prevents WebSocket timeout when the app is in background.
 */
function startKeepalive(): void {
  if (keepaliveTimer) return;
  keepaliveTimer = window.setInterval(() => {
    if (socket && socket.readyState === WebSocket.OPEN) {
      // Send a lightweight ping message
      // Note: The server should ignore unknown message types
      try {
        socket.send(JSON.stringify({ type: 'ping' }));
      } catch {
        // Socket may have closed, will be handled by close event
      }
    }
  }, KEEPALIVE_INTERVAL_MS);
}

function stopKeepalive(): void {
  if (keepaliveTimer) {
    window.clearInterval(keepaliveTimer);
    keepaliveTimer = null;
  }
}

/**
 * Handle visibility change - when app comes back to foreground.
 * Immediately verify WebSocket connection and reconnect if needed.
 */
function handleVisibilityChange(): void {
  if (document.visibilityState === 'visible') {
    // App is now visible - verify WebSocket is still connected
    if (socket && socket.readyState !== WebSocket.OPEN) {
      console.log('[httpEvents] WebSocket disconnected while in background, reconnecting...');
      socket = null;
      openPromise = null;
      reconnectCount = 0; // Reset count for immediate reconnect
      if (handlers.size > 0 || keepAlive) {
        ensureSocket().catch(() => {
          // Connection errors handled by WebSocket event listeners
        });
      }
    }
  }
}

// Register visibility change handler
if (typeof document !== 'undefined') {
  document.addEventListener('visibilitychange', handleVisibilityChange);
}

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
      }
    }
  }

  openPromise = new Promise((resolve) => {
    socket = new WebSocket(wsUrl);

    // Create named handlers so we can remove them later
    const handleOpen = () => {
      openPromise = null;
      notifyConnected();
      startKeepalive(); // Start keepalive pings for background tab support
      resolve();
    };

    const handleMessage = (event: MessageEvent) => {
      if (!event.data) return;
      const rawData = event.data as string;

      // Check if this is an audio_levels event (quick string check)
      const isAudioLevels = rawData.includes('"event":"audio_levels"');

      // Forward audio data to worker for off-thread canvas rendering
      if (isAudioLevels && isWorkerReady()) {
        forwardAudioData(rawData);
      }

      // Parse and dispatch to main thread handlers
      // NOTE: audio_levels is still parsed for the main thread store update,
      // which is needed for DOM updates (peak dB display) even when worker
      // handles canvas rendering. The store update is fast (just object mutation).
      try {
        const parsed = JSON.parse(rawData) as {
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
    };

    const handleClose = (event: CloseEvent) => {
      // CRITICAL: Clean up listeners before nullifying socket to prevent memory leaks
      cleanupSocketListeners();
      socket = null;
      openPromise = null;
      stopKeepalive(); // Stop keepalive pings

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
    };

    const handleError = () => {
      if (socket && socket.readyState === WebSocket.OPEN) {
        return;
      }
      // Clean up listeners if socket is closing
      cleanupSocketListeners();
      socket = null;
      openPromise = null;
      notifyDisconnected('Connection error');
      if (handlers.size > 0 || keepAlive) {
        scheduleReconnect();
      }
    };

    // Store references for cleanup
    currentListeners = {
      open: handleOpen,
      message: handleMessage,
      close: handleClose,
      error: handleError,
    };

    // Add listeners
    socket.addEventListener('open', handleOpen);
    socket.addEventListener('message', handleMessage);
    socket.addEventListener('close', handleClose);
    socket.addEventListener('error', handleError);
  });

  return openPromise;
}

function scheduleReconnect() {
  if (reconnectTimer) return;

  // Check if we've exceeded max reconnection attempts
  if (reconnectCount >= MAX_RECONNECT_ATTEMPTS) {
    notifyDisconnected('Connection lost. Please refresh the page.');
    return;
  }

  reconnectCount++;

  // First 3 attempts use faster retry (200ms) for quick recovery from startup race conditions
  // After that, use exponential backoff: 1s, 1.5s, 2.25s, ... up to 30s max
  const delay = reconnectCount <= 3
    ? 200
    : Math.min(
        INITIAL_RECONNECT_DELAY * Math.pow(1.5, reconnectCount - 4),
        MAX_RECONNECT_DELAY
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
 *
 * A small delay is added before the first connection attempt to avoid
 * race conditions where the health check passes but the WebSocket handler
 * isn't quite ready yet (common on macOS/Safari).
 */
export function initConnection(): void {
  keepAlive = true;
  // Small delay before initial connection to avoid race condition with server startup
  // The health check passes before WebSocket upgrade handler is fully ready
  setTimeout(() => {
    ensureSocket().catch(() => {
      // Connection errors handled by WebSocket event listeners
    });
  }, 100);
}

/**
 * Disconnect and clear the WebSocket connection.
 * Call this when logging out to clean up.
 */
export function disconnectSocket(): void {
  keepAlive = false;
  reconnectCount = 0; // Reset counter for fresh start on next connection
  stopKeepalive(); // Stop keepalive pings
  if (reconnectTimer) {
    window.clearTimeout(reconnectTimer);
    reconnectTimer = null;
  }
  if (socket) {
    // Clean up listeners before closing to prevent memory leaks
    cleanupSocketListeners();
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
