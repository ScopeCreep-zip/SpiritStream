import { create } from 'zustand';

export type ConnectionStatus = 'connected' | 'connecting' | 'disconnected';

interface ConnectionState {
  /** Current WebSocket state (`ws://.../api/v1/events`). */
  status: ConnectionStatus;
  /** Timestamp of the most recent successful connect. `null` until the
   *  first connect ever completes — used by the badge to distinguish
   *  "first connect in progress" from "reconnecting after a drop". */
  lastConnected: Date | null;
  /** Last error string from the underlying socket, if any. */
  error: string | null;

  setConnected: () => void;
  setDisconnected: (error?: string) => void;
  setConnecting: () => void;
}

export const useConnectionStore = create<ConnectionState>((set) => ({
  status: 'disconnected',
  lastConnected: null,
  error: null,

  setConnected: () =>
    set({
      status: 'connected',
      lastConnected: new Date(),
      error: null,
    }),

  setDisconnected: (error) =>
    set({
      status: 'disconnected',
      error: error || null,
    }),

  setConnecting: () =>
    set({
      status: 'connecting',
    }),
}));
