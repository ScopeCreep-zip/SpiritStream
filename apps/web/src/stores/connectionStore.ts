import { create } from 'zustand';
import { backendMode } from '@/lib/backend/env';

export type ConnectionStatus = 'connected' | 'connecting' | 'disconnected';

interface ConnectionState {
  status: ConnectionStatus;
  lastConnected: Date | null;
  reconnectAttempts: number;
  error: string | null;

  // Actions
  setStatus: (status: ConnectionStatus) => void;
  setConnected: () => void;
  setDisconnected: (error?: string) => void;
  setConnecting: () => void;
  incrementReconnectAttempts: () => void;
  resetReconnectAttempts: () => void;
}

export const useConnectionStore = create<ConnectionState>((set, get) => ({
  // In Tauri mode, we're always "connected" since it's local IPC
  status: backendMode === 'tauri' ? 'connected' : 'disconnected',
  lastConnected: backendMode === 'tauri' ? new Date() : null,
  reconnectAttempts: 0,
  error: null,

  setStatus: (status) => set({ status }),

  setConnected: () =>
    set({
      status: 'connected',
      lastConnected: new Date(),
      reconnectAttempts: 0,
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

  incrementReconnectAttempts: () =>
    set({
      reconnectAttempts: get().reconnectAttempts + 1,
    }),

  resetReconnectAttempts: () =>
    set({
      reconnectAttempts: 0,
    }),
}));
