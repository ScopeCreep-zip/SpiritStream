import { create } from 'zustand';
import { api } from '@/lib/client';
import { logger } from '@/lib/logger';
import { showSystemNotification } from '@/lib/notification';
import { useSettingsStore } from './settingsStore';
import i18n from '@/lib/i18n';
import type { ObsConnectionStatus, ObsStreamStatus, ObsState } from '@spiritstream/types';

/**
 * Runtime/connection state for OBS only. OBS *settings* (host/port/password/
 * useAuth/direction/autoConnect) are owned by the active profile — the single
 * source of truth. The form reads them from `current.settings.obs` and writes
 * them via `updateProfileSettings({ obs })`; the backend syncs its handler from
 * the profile on activation/save (`apply_profile_obs`). This store holds no
 * config mirror, so it can never drift from the profile.
 */
interface ObsStoreState {
  // Connection/runtime state
  connectionStatus: ObsConnectionStatus;
  streamStatus: ObsStreamStatus;
  errorMessage: string | null;
  obsVersion: string | null;
  websocketVersion: string | null;

  // UI state (transient, client-only)
  showPassword: boolean;

  // Actions
  setShowPassword: (show: boolean) => void;
  loadState: () => Promise<void>;
  connect: () => Promise<void>;
  disconnect: () => Promise<void>;
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
  showPassword: false,

  setShowPassword: (show) => set({ showPassword: show }),

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
      logger.error('Failed to load OBS state:', error);
    }
  },

  connect: async () => {
    // Backend owns auto-reconnect (`ObsWebSocketHandler::spawn_auto_connect`),
    // so the frontend no longer signals "this is a manual connect".
    try {
      set({ connectionStatus: 'connecting', errorMessage: null });
      await api.obs.connect();
      // State will be updated via WebSocket events
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      set({ connectionStatus: 'error', errorMessage: message });
      throw error;
    }
  },

  disconnect: async () => {
    try {
      await api.obs.disconnect();
      set({
        connectionStatus: 'disconnected',
        streamStatus: 'unknown',
        obsVersion: null,
        websocketVersion: null,
        errorMessage: null,
      });
    } catch (error) {
      logger.error('Failed to disconnect from OBS:', error);
      throw error;
    }
  },

  startStream: async () => {
    try {
      await api.obs.startStream();
    } catch (error) {
      logger.error('Failed to start OBS stream:', error);
      throw error;
    }
  },

  stopStream: async () => {
    try {
      await api.obs.stopStream();
    } catch (error) {
      logger.error('Failed to stop OBS stream:', error);
      throw error;
    }
  },

  updateFromEvent: (state) => {
    const prevConnectionStatus = get().connectionStatus;
    const newConnectionStatus = state.connectionStatus ?? prevConnectionStatus;

    set({
      connectionStatus: newConnectionStatus,
      streamStatus: state.streamStatus ?? get().streamStatus,
      errorMessage: state.errorMessage ?? get().errorMessage,
      obsVersion: state.obsVersion ?? get().obsVersion,
      websocketVersion: state.websocketVersion ?? get().websocketVersion,
    });

    // Notify on OBS connection state changes (only actual connect/disconnect, not errors)
    const showNotifications = useSettingsStore.getState().showNotifications;
    if (showNotifications && newConnectionStatus !== prevConnectionStatus) {
      if (newConnectionStatus === 'connected' && prevConnectionStatus !== 'connected') {
        showSystemNotification(
          i18n.t('notifications.obsConnectedTitle', 'OBS Connected'),
          i18n.t('notifications.obsConnectedBody', 'Successfully connected to OBS WebSocket.')
        );
      } else if (newConnectionStatus === 'disconnected' && prevConnectionStatus === 'connected') {
        showSystemNotification(
          i18n.t('notifications.obsDisconnectedTitle', 'OBS Disconnected'),
          i18n.t('notifications.obsDisconnectedBody', 'Disconnected from OBS WebSocket.')
        );
      }
    }
  },
}));
