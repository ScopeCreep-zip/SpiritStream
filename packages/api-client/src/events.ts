import { getBackendWsUrl } from './config';
import { eventsTicketIssue } from './generated';

// Connection state changes are surfaced through `window` CustomEvents so
// frontends can listen without taking a dependency on a specific state
// management library. The shipped Zustand store
// (`apps/web/src/stores/connectionStore.ts`) subscribes to these.
// `detail.error` carries a stable CODE (`auth_required`,
// `connection_error`, `max_reconnects`) — the UI layer owns the i18n.
type ConnectionDetail = { error?: string };
const dispatch = (name: string, detail?: ConnectionDetail) => {
  if (typeof window !== 'undefined') {
    window.dispatchEvent(new CustomEvent(`backend:${name}`, { detail }));
  }
};
// Lightweight console logger keeps the package transport-agnostic.
const logger = {
  warn: (...args: unknown[]) => console.warn('[api-client]', ...args),
  error: (...args: unknown[]) => console.error('[api-client]', ...args),
};

// Local mirror of reconnect state — the store side reads it from
// `backend:status` events.
let reconnectAttempts = 0;

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
  // "Was this a reconnect after a prior successful connection?" — the
  // signal is `wasConnected`, NOT `reconnectAttempts > 0`. The counter
  // is incremented by `notifyConnecting()` on every attempt, including
  // the very first one, so using it here mis-classified first connects
  // as reconnects and produced a spurious "Backend reconnected" toast
  // on every fresh launch. `wasConnected` stays false until we've
  // actually connected once, which is the real semantic we want.
  const wasReconnecting = wasConnected;
  wasConnected = true;
  reconnectAttempts = 0;
  reconnectCount = 0;
  dispatch(wasReconnecting ? 'reconnected' : 'connected');
}

function notifyDisconnected(error?: string) {
  if (wasConnected) {
    dispatch('disconnected', { error });
  }
}

function notifyConnecting() {
  reconnectAttempts++;
  dispatch('connecting');
}

function notifyAuthRequired() {
  dispatch('auth-required');
}

/**
 * Build the upgrade URL. Cross-origin connections (UI served from a
 * different host than the API) can't ride the session cookie on a WS
 * upgrade, so we fetch a one-shot, seconds-lived ticket from the
 * authenticated REST endpoint and pass it as `?ticket=`. (The previous
 * design read a long-lived bearer token out of localStorage — which no
 * production code ever wrote, and which would have leaked into proxy
 * logs if it had.)
 */
async function buildWsUrl(): Promise<string> {
  let wsUrl = getBackendWsUrl();
  if (typeof window !== 'undefined') {
    const wsHost = new URL(wsUrl.replace('ws://', 'http://').replace('wss://', 'https://')).host;
    const isCrossOrigin = wsHost !== window.location.host;
    if (isCrossOrigin) {
      try {
        const { data } = await eventsTicketIssue({ throwOnError: true });
        const separator = wsUrl.includes('?') ? '&' : '?';
        wsUrl = `${wsUrl}${separator}ticket=${encodeURIComponent(data.ticket)}`;
      } catch (error) {
        // Not authenticated yet (or backend briefly down): connect
        // without a ticket and let the upgrade's 401 drive the
        // auth-required flow.
        logger.warn('events ticket fetch failed — connecting without one:', error);
      }
    }
  }
  return wsUrl;
}

function ensureSocket(): Promise<void> {
  if (socket && socket.readyState === WebSocket.OPEN) {
    return Promise.resolve();
  }

  if (openPromise) {
    return openPromise;
  }

  notifyConnecting();

  const attempt = buildWsUrl().then(
    (wsUrl) =>
      new Promise<void>((resolve, reject) => {
        // Pre-fix, this promise only ever resolved (on 'open'). A socket
        // that errored before opening left every `await events.on(...)`
        // pending forever — effect cleanups never received their
        // unlisten functions, so handlers leaked. Now a pre-open
        // close settles the promise with a coded rejection.
        let settled = false;
        socket = new WebSocket(wsUrl);

        socket.addEventListener('open', () => {
          settled = true;
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
            logger.warn('Failed to parse backend event:', error);
          }
        });

        // Per the WebSocket spec a 'close' always follows 'error', so all
        // teardown lives here — the old separate 'error' teardown ran the
        // same work twice and double-dispatched `backend:disconnected`.
        socket.addEventListener('close', (event) => {
          socket = null;
          openPromise = null;

          // Auth failure (401 on upgrade surfaces as policy-violation 1008).
          const authFailure = event.code === 1008 || event.reason === 'Unauthorized';
          if (authFailure) {
            notifyAuthRequired();
            notifyDisconnected('auth_required');
          } else {
            notifyDisconnected();
          }

          if (!settled) {
            settled = true;
            reject(new Error(authFailure ? 'auth_required' : 'connection_error'));
          }

          // Reconnecting on an auth failure would loop 401s forever; the
          // auth-required flow owns recovery there.
          if (!authFailure && (handlers.size > 0 || keepAlive)) {
            scheduleReconnect();
          }
        });
      })
  );

  // Clear the slot when the attempt fails (including buildWsUrl itself
  // throwing) so the next ensureSocket() starts fresh instead of
  // returning a permanently-rejected promise.
  openPromise = attempt.catch((error) => {
    openPromise = null;
    throw error;
  });

  return openPromise;
}

function scheduleReconnect() {
  if (reconnectTimer) return;

  // Check if we've exceeded max reconnection attempts
  if (reconnectCount >= MAX_RECONNECT_ATTEMPTS) {
    notifyDisconnected('max_reconnects');
    return;
  }

  reconnectCount++;

  // Exponential backoff: 1s, 1.5s, 2.25s, ... up to 30s max
  const delay = Math.min(
    INITIAL_RECONNECT_DELAY * Math.pow(1.5, reconnectCount - 1),
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
 */
export function initConnection(): void {
  keepAlive = true;
  ensureSocket().catch(() => {
    // Connection errors handled by WebSocket event listeners
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

    try {
      await ensureSocket();
    } catch (error) {
      // The subscribe failed and the caller never receives an unlisten
      // function — remove the handler we just added so it can't leak.
      const listeners = handlers.get(eventName);
      if (listeners) {
        listeners.delete(handler as AnyHandler);
        if (listeners.size === 0) {
          handlers.delete(eventName);
        }
      }
      throw error;
    }

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
