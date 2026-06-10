import type { StateCreator } from 'zustand';
import { api } from '@/lib/client';
import { logger } from '@/lib/logger';
import type { StreamStatusType } from '@/types/stream';
import { initialStats, type StreamState } from './types';

// SpiritStream→OBS triggering lives in core
// (`FFmpegHandler::fire_obs_trigger` →
// `ObsWebSocketHandler::ss_trigger_obs`). Every successful
// `start_all` / `stop_all` checks the active profile's
// `obs.direction` server-side and either drives OBS or no-ops. The
// frontend never has to decide.

type CoreSlice = Pick<
  StreamState,
  | 'isStreaming'
  | 'activeGroups'
  | 'enabledGroups'
  | 'enabledTargets'
  | 'activeStreamCount'
  | 'error'
  | 'syncWithBackend'
  | 'startGroup'
  | 'stopGroup'
  | 'startAllGroups'
  | 'stopAllGroups'
  | 'toggleTargetLive'
  | 'setIsStreaming'
  | 'setGroupEnabled'
  | 'setTargetEnabled'
  | 'setError'
  | 'reset'
>;

export const createCoreSlice: StateCreator<StreamState, [], [], CoreSlice> = (set, get) => ({
  isStreaming: false,
  activeGroups: new Set(),
  enabledGroups: new Set(),
  enabledTargets: new Set(),
  error: null,
  activeStreamCount: 0,

  syncWithBackend: async () => {
    try {
      const [activeCount, activeGroupIds] = await Promise.all([
        api.stream.getActiveCount(),
        api.stream.getActiveGroupIds(),
      ]);

      const activeGroups = new Set(activeGroupIds);
      const isStreaming = activeCount > 0;

      set({
        activeStreamCount: activeCount,
        activeGroups,
        isStreaming,
        groupStats: isStreaming ? get().groupStats : {},
        stats: isStreaming ? get().stats : initialStats,
        uptime: isStreaming ? get().uptime : 0,
        globalStatus: isStreaming ? 'live' : 'offline',
      });
    } catch (error) {
      logger.error('[StreamStore] Failed to sync with backend:', error);
    }
  },

  startGroup: async (group, incomingUrl) => {
    set({ globalStatus: 'connecting' });
    get().setError(null);
    try {
      await api.stream.start(group, incomingUrl);
      const activeGroups = new Set(get().activeGroups);
      activeGroups.add(group.id);
      set({
        activeGroups,
        isStreaming: true,
      });
      // Start the displayed uptime at 0 so the StatusStrip clock
      // begins ticking from 00:00 — `useStreamStats` runs a 1Hz
      // optimistic tick (`incrementUptime`) and `updateStats`
      // overwrites with the ffmpeg `time` value on each stats event.
      get().setUptime(0);
      get().setGlobalStatus('live');
      // SS→OBS trigger runs server-side in core (see top of file).
    } catch (error) {
      set({ globalStatus: 'error' });
      get().setError(String(error));
    }
  },

  stopGroup: async (groupId) => {
    try {
      await api.stream.stop(groupId);
      const activeGroups = new Set(get().activeGroups);
      activeGroups.delete(groupId);
      const isStreaming = activeGroups.size > 0;
      set({
        activeGroups,
        isStreaming,
      });
      get().setGlobalStatus(isStreaming ? 'live' : 'offline');
    } catch (error) {
      get().setError(String(error));
    }
  },

  // Backend handles filtering disabled targets via disabled_targets set.
  startAllGroups: async (groups, incomingUrl) => {
    set({ globalStatus: 'connecting' });
    get().setError(null);

    try {
      // Filter groups by: has targets AND is enabled.
      // If enabledGroups is empty, treat all groups as enabled (first-time startup case).
      const enabledGroups = get().enabledGroups;
      const eligibleGroups = groups.filter((group) => {
        const hasTargets = group.streamTargets.length > 0;
        const isEnabled = enabledGroups.size === 0 || enabledGroups.has(group.id);
        return hasTargets && isEnabled;
      });

      if (eligibleGroups.length === 0) {
        throw new Error('At least one enabled output group with stream targets is required');
      }

      await api.stream.startAll(eligibleGroups, incomingUrl);

      const activeGroups = new Set(get().activeGroups);
      for (const group of eligibleGroups) {
        activeGroups.add(group.id);
      }
      set({ activeGroups, isStreaming: true });
      // Reset displayed uptime — see `startGroup` for the rationale.
      get().setUptime(0);
      get().setGlobalStatus('live');
    } catch (error) {
      set({ globalStatus: 'error' });
      get().setError(String(error));
      throw error;
    }
  },

  stopAllGroups: async () => {
    try {
      await api.stream.stopAll();
      set({
        activeGroups: new Set(),
        isStreaming: false,
        uptime: 0,
        groupStats: {},
        stats: initialStats,
      });
      get().setGlobalStatus('offline');
    } catch (error) {
      get().setError(String(error));
    }
  },

  toggleTargetLive: async (targetId, enabled, group, incomingUrl) => {
    try {
      await api.stream.toggleTarget(targetId, enabled, group, incomingUrl);

      const enabledTargets = new Set(get().enabledTargets);
      if (enabled) {
        enabledTargets.add(targetId);
      } else {
        enabledTargets.delete(targetId);
      }
      set({ enabledTargets });
    } catch (error) {
      get().setError(String(error));
      throw error;
    }
  },

  setIsStreaming: (isStreaming) => {
    const status: StreamStatusType = isStreaming ? 'live' : 'offline';
    set({ isStreaming, globalStatus: status });
  },

  setGroupEnabled: (groupId, enabled) => {
    const enabledGroups = new Set(get().enabledGroups);
    if (enabled) {
      enabledGroups.add(groupId);
    } else {
      enabledGroups.delete(groupId);
    }
    set({ enabledGroups });
  },

  setTargetEnabled: (targetId, enabled) => {
    const enabledTargets = new Set(get().enabledTargets);
    if (enabled) {
      enabledTargets.add(targetId);
    } else {
      enabledTargets.delete(targetId);
    }
    set({ enabledTargets });
  },

  setError: (error) => set({ error }),

  reset: () => {
    set({
      isStreaming: false,
      activeGroups: new Set(),
      stats: initialStats,
      groupStats: {},
      uptime: 0,
      globalStatus: 'offline' as StreamStatusType,
      activeStreamCount: 0,
    });
    get().setError(null);
  },
});
