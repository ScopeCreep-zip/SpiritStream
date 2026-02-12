import { create } from 'zustand';
import { backendMode } from '@/lib/backend/env';
import { api } from '@/lib/backend';

export type ConnectionStatus = 'connected' | 'connecting' | 'disconnected';

// Health check interval once connected (30 seconds — server is local, no need to poll frequently)
const CONNECTED_HEALTH_CHECK_INTERVAL = 30000;
// Initial retry interval on failure (exponential backoff: 1s → 2s → 4s → 8s → 10s cap)
const INITIAL_RETRY_INTERVAL = 1000;
const MAX_RETRY_INTERVAL = 10000;
// Maximum consecutive failures before marking as disconnected
const MAX_FAILURES_BEFORE_DISCONNECT = 3;

interface ConnectionState {
  status: ConnectionStatus;
  lastConnected: Date | null;
  reconnectAttempts: number;
  error: string | null;
  /** Number of consecutive health check failures in Tauri mode */
  tauriHealthCheckFailures: number;
  /** Interval ID for Tauri health check */
  healthCheckIntervalId: ReturnType<typeof setInterval> | null;

  // Actions
  setStatus: (status: ConnectionStatus) => void;
  setConnected: () => void;
  setDisconnected: (error?: string) => void;
  setConnecting: () => void;
  incrementReconnectAttempts: () => void;
  resetReconnectAttempts: () => void;
  /** Start health check polling for Tauri mode */
  startTauriHealthCheck: () => void;
  /** Stop health check polling */
  stopTauriHealthCheck: () => void;
  /** Perform a single health check */
  checkHealth: () => Promise<boolean>;
}

export const useConnectionStore = create<ConnectionState>((set, get) => ({
  // In Tauri mode, start as connecting until first health check
  status: backendMode === 'tauri' ? 'connecting' : 'disconnected',
  lastConnected: null,
  reconnectAttempts: 0,
  error: null,
  tauriHealthCheckFailures: 0,
  healthCheckIntervalId: null,

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

  checkHealth: async () => {
    try {
      await api.system.health();
      // Reset failures on success
      set({
        status: 'connected',
        lastConnected: new Date(),
        tauriHealthCheckFailures: 0,
        error: null,
      });
      return true;
    } catch (err) {
      const failures = get().tauriHealthCheckFailures + 1;
      set({ tauriHealthCheckFailures: failures });

      // Only mark as disconnected after multiple consecutive failures
      if (failures >= MAX_FAILURES_BEFORE_DISCONNECT) {
        set({
          status: 'disconnected',
          error: 'Backend server not responding',
        });
      }
      return false;
    }
  },

  startTauriHealthCheck: () => {
    // Only run in Tauri mode
    if (backendMode !== 'tauri') return;

    // Clear any existing interval
    const existingInterval = get().healthCheckIntervalId;
    if (existingInterval) {
      clearInterval(existingInterval);
    }

    // Do an immediate check (no initial delay)
    get().checkHealth();

    // Adaptive interval: 30s when connected, exponential backoff on failure
    const scheduleNext = (): void => {
      const state = get();
      const isConnected = state.status === 'connected';
      const failures = state.tauriHealthCheckFailures;

      const interval = isConnected
        ? CONNECTED_HEALTH_CHECK_INTERVAL
        : Math.min(INITIAL_RETRY_INTERVAL * Math.pow(2, failures), MAX_RETRY_INTERVAL);

      const intervalId = setTimeout(async () => {
        await get().checkHealth();
        scheduleNext();
      }, interval);

      set({ healthCheckIntervalId: intervalId as unknown as ReturnType<typeof setInterval> });
    };

    scheduleNext();
  },

  stopTauriHealthCheck: () => {
    const intervalId = get().healthCheckIntervalId;
    if (intervalId) {
      clearInterval(intervalId);
      set({ healthCheckIntervalId: null });
    }
  },
}));

// Auto-start health check in Tauri mode when the module loads (immediate, no delay)
if (typeof window !== 'undefined' && backendMode === 'tauri') {
  useConnectionStore.getState().startTauriHealthCheck();
}
