import { getBackendWsUrl } from './env';

type Handler<T> = (payload: T) => void;

type AnyHandler = Handler<unknown>;

const handlers = new Map<string, Set<AnyHandler>>();
let socket: WebSocket | null = null;
let openPromise: Promise<void> | null = null;
let reconnectTimer: number | null = null;

function ensureSocket(): Promise<void> {
  if (socket && socket.readyState === WebSocket.OPEN) {
    return Promise.resolve();
  }

  if (openPromise) {
    return openPromise;
  }

  openPromise = new Promise((resolve) => {
    socket = new WebSocket(getBackendWsUrl());

    socket.addEventListener('open', () => {
      openPromise = null;
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

    socket.addEventListener('close', () => {
      socket = null;
      openPromise = null;
      if (handlers.size > 0) {
        scheduleReconnect();
      }
    });

    socket.addEventListener('error', () => {
      if (socket && socket.readyState === WebSocket.OPEN) {
        return;
      }
      socket = null;
      openPromise = null;
      if (handlers.size > 0) {
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

      if (handlers.size === 0 && socket) {
        socket.close();
        socket = null;
      }
    };
  },
};
