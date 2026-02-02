import { create } from 'zustand';
import { api } from '@/lib/backend/httpApi';
import { showSystemNotification } from '@/lib/notification';
import { useSettingsStore } from './settingsStore';
import { useProfileStore } from './profileStore';
import i18n from '@/lib/i18n';
import type {
  ObsConnectionStatus,
  ObsStreamStatus,
  ObsConfig,
  ObsState,
} from '@/types/api';
import type { ObsSettings } from '@/types/profile';

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
  syncConfigFromProfile: () => void;
  updateConfig: (config: Partial<ObsConfig> & { password?: string }) => Promise<void>;
  connect: (isManual?: boolean) => Promise<void>;
  disconnect: (isManual?: boolean) => Promise<void>;
  startStream: () => Promise<void>;
  stopStream: () => Promise<void>;
  updateFromEvent: (state: Partial<ObsState>) => void;
}

/**
 * Convert profile ObsSettings to ObsConfig format
 */
const obsSettingsToConfig = (settings: ObsSettings): ObsConfig => ({
  host: settings.host,
  port: settings.port,
  password: settings.password,
  useAuth: settings.useAuth,
  direction: settings.direction,
  autoConnect: settings.autoConnect,
  hasPassword: settings.password.length > 0,
});

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

  /**
   * Load config from backend API (legacy, for initial load)
   * This will be replaced by syncConfigFromProfile when a profile is loaded
   */
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

  /**
   * Sync OBS config from the current profile's settings
   * Called when a profile is loaded or profile settings change
   * Also updates the backend's in-memory OBS handler
   */
  syncConfigFromProfile: () => {
    const current = useProfileStore.getState().current;
    if (current?.settings?.obs) {
      const config = obsSettingsToConfig(current.settings.obs);
      set({ config });

      // Also update the backend's in-memory OBS handler so it uses the profile's config
      api.obs.setConfig({
        host: config.host,
        port: config.port,
        password: current.settings.obs.password,
        useAuth: config.useAuth,
        direction: config.direction,
        autoConnect: config.autoConnect,
      }).catch((error) => {
        console.error('Failed to sync OBS config to backend:', error);
      });
    }
  },

  /**
   * Update OBS config - saves to current profile AND updates backend
   */
  updateConfig: async (updates) => {
    const currentConfig = get().config;
    const currentProfile = useProfileStore.getState().current;

    if (!currentConfig || !currentProfile?.settings) {
      console.error('Cannot update OBS config: no config or profile loaded');
      return;
    }

    try {
      set({ isLoading: true });

      // Build the new config
      const newConfig: ObsConfig = {
        host: updates.host ?? currentConfig.host,
        port: updates.port ?? currentConfig.port,
        password: updates.password ?? currentConfig.password,
        useAuth: updates.useAuth ?? currentConfig.useAuth,
        direction: updates.direction ?? currentConfig.direction,
        autoConnect: updates.autoConnect ?? currentConfig.autoConnect,
      };

      // Update local state immediately
      set({ config: newConfig });

      // Update the profile's OBS settings
      const newObsSettings: ObsSettings = {
        host: newConfig.host,
        port: newConfig.port,
        password: newConfig.password,
        useAuth: newConfig.useAuth,
        direction: newConfig.direction,
        autoConnect: newConfig.autoConnect,
      };

      // Update profile settings (this will save the profile)
      await useProfileStore.getState().updateProfileSettings({
        obs: newObsSettings,
      });

      // Also update the backend's in-memory OBS handler
      // (so the WebSocket connection uses the new config)
      await api.obs.setConfig({
        host: newConfig.host,
        port: newConfig.port,
        password: updates.password, // Pass original password for encryption
        useAuth: newConfig.useAuth,
        direction: newConfig.direction,
        autoConnect: newConfig.autoConnect,
      });
    } catch (error) {
      console.error('Failed to update OBS config:', error);
      // Reload config from profile on error
      get().syncConfigFromProfile();
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
