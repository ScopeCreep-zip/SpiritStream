import { create } from 'zustand';
import { api } from '@/lib/backend/httpApi';
import type {
  ObsConnectionStatus,
  ObsStreamStatus,
  ObsConfig,
  ObsState,
} from '@/types/api';

interface ObsStoreState {
  // Connection state
  connectionStatus: ObsConnectionStatus;
  streamStatus: ObsStreamStatus;
  errorMessage: string | null;
  obsVersion: string | null;
  websocketVersion: string | null;

  // Configuration
  config: ObsConfig | null;
  isLoading: boolean;

  // UI state
  showPassword: boolean;

  // Integration state
  // When true, the next OBS stream state change was triggered by SpiritStream
  // and should not trigger SpiritStream back (prevents loops)
  triggeredByUs: boolean;

  // Actions
  setShowPassword: (show: boolean) => void;
  setTriggeredByUs: (value: boolean) => void;
  loadState: () => Promise<void>;
  loadConfig: () => Promise<void>;
  updateConfig: (config: Partial<ObsConfig> & { password?: string }) => Promise<void>;
  connect: (isManual?: boolean) => Promise<void>;
  disconnect: (isManual?: boolean) => Promise<void>;
  startStream: () => Promise<void>;
  stopStream: () => Promise<void>;
  updateFromEvent: (state: Partial<ObsState>) => void;
}

export const useObsStore = create<ObsStoreState>((set, get) => ({
  // Initial state
  connectionStatus: 'disconnected',
  streamStatus: 'unknown',
  errorMessage: null,
  obsVersion: null,
  websocketVersion: null,
  config: null,
  isLoading: false,
  showPassword: false,
  triggeredByUs: false,

  setShowPassword: (show) => set({ showPassword: show }),
  setTriggeredByUs: (value) => set({ triggeredByUs: value }),

  loadState: async () => {
    try {
      const state = await api.obs.getState();
      set({
        connectionStatus: state.connectionStatus,
        streamStatus: state.streamStatus,
        errorMessage: state.errorMessage,
        obsVersion: state.obsVersion,
        websocketVersion: state.websocketVersion,
      });
    } catch (error) {
      console.error('Failed to load OBS state:', error);
    }
  },

  loadConfig: async () => {
    try {
      set({ isLoading: true });
      const config = await api.obs.getConfig();
      set({ config });
    } catch (error) {
      console.error('Failed to load OBS config:', error);
    } finally {
      set({ isLoading: false });
    }
  },

  updateConfig: async (updates) => {
    const currentConfig = get().config;
    if (!currentConfig) return;

    try {
      set({ isLoading: true });

      await api.obs.setConfig({
        host: updates.host ?? currentConfig.host,
        port: updates.port ?? currentConfig.port,
        password: updates.password,
        useAuth: updates.useAuth ?? currentConfig.useAuth,
        direction: updates.direction ?? currentConfig.direction,
        autoConnect: updates.autoConnect ?? currentConfig.autoConnect,
      });

      // Reload config to get the updated values
      await get().loadConfig();
    } catch (error) {
      console.error('Failed to update OBS config:', error);
      throw error;
    } finally {
      set({ isLoading: false });
    }
  },

  connect: async (isManual = true) => {
    try {
      // Notify that this is a manual connect (re-enables auto-reconnect)
      if (isManual) {
        window.dispatchEvent(new CustomEvent('obs:manual-connect'));
      }
      set({ connectionStatus: 'connecting', errorMessage: null });
      await api.obs.connect();
      // State will be updated via WebSocket events
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      set({
        connectionStatus: 'error',
        errorMessage: message,
      });
      throw error;
    }
  },

  disconnect: async (isManual = true) => {
    try {
      // Notify that this is a manual disconnect (disables auto-reconnect)
      if (isManual) {
        window.dispatchEvent(new CustomEvent('obs:manual-disconnect'));
      }
      await api.obs.disconnect();
      set({
        connectionStatus: 'disconnected',
        streamStatus: 'unknown',
        obsVersion: null,
        websocketVersion: null,
        errorMessage: null,
      });
    } catch (error) {
      console.error('Failed to disconnect from OBS:', error);
      throw error;
    }
  },

  startStream: async () => {
    try {
      await api.obs.startStream();
    } catch (error) {
      console.error('Failed to start OBS stream:', error);
      throw error;
    }
  },

  stopStream: async () => {
    try {
      await api.obs.stopStream();
    } catch (error) {
      console.error('Failed to stop OBS stream:', error);
      throw error;
    }
  },

  updateFromEvent: (state) => {
    set({
      connectionStatus: state.connectionStatus ?? get().connectionStatus,
      streamStatus: state.streamStatus ?? get().streamStatus,
      errorMessage: state.errorMessage ?? get().errorMessage,
      obsVersion: state.obsVersion ?? get().obsVersion,
      websocketVersion: state.websocketVersion ?? get().websocketVersion,
    });
  },
}));
