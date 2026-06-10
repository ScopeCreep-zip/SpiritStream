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
  | 'liveTargetOverrides'
  | 'activeStreamCount'
  | 'syncWithBackend'
  | 'startGroup'
  | 'stopGroup'
  | 'startAllGroups'
  | 'stopAllGroups'
  | 'toggleTargetLive'
  | 'setIsStreaming'
  | 'reset'
>;

export const createCoreSlice: StateCreator<StreamState, [], [], CoreSlice> = (set, get) => ({
  isStreaming: false,
  activeGroups: new Set(),
  liveTargetOverrides: new Map(),
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
      // Rethrow so callers surface the failure — swallowing here let
      // "Streaming {{name}}" success toasts fire on failed starts.
      throw error;
    }
  },

  // A failed stop propagates to the caller — a "Stopped" toast on
  // failure would mean FFmpeg is still pushing while the user believes
  // they're offline.
  stopGroup: async (groupId) => {
    await api.stream.stop(groupId);
    const activeGroups = new Set(get().activeGroups);
    activeGroups.delete(groupId);
    const isStreaming = activeGroups.size > 0;
    set({
      activeGroups,
      isStreaming,
      // Live overrides only make sense while something is live.
      liveTargetOverrides: isStreaming ? get().liveTargetOverrides : new Map(),
    });
    get().setGlobalStatus(isStreaming ? 'live' : 'offline');
  },

  // Eligibility (group.enabled + at least one enabled target) is decided
  // SERVER-side: send every group, render core's verdict. Core answers
  // with `startedGroupIds` (the authority) or a `no_eligible_groups`
  // validation error.
  startAllGroups: async (groups, incomingUrl) => {
    set({ globalStatus: 'connecting' });
    try {
      const { startedGroupIds } = await api.stream.startAll(groups, incomingUrl);

      const activeGroups = new Set(get().activeGroups);
      for (const groupId of startedGroupIds) {
        activeGroups.add(groupId);
      }
      set({ activeGroups, isStreaming: true, liveTargetOverrides: new Map() });
      // Reset displayed uptime — see `startGroup` for the rationale.
      get().setUptime(0);
      get().setGlobalStatus('live');
    } catch (error) {
      set({ globalStatus: 'error' });
      throw error;
    }
  },

  stopAllGroups: async () => {
    await api.stream.stopAll();
    set({
      activeGroups: new Set(),
      liveTargetOverrides: new Map(),
      isStreaming: false,
      uptime: 0,
      groupStats: {},
      stats: initialStats,
    });
    get().setGlobalStatus('offline');
  },

  toggleTargetLive: async (targetId, enabled, group, incomingUrl) => {
    await api.stream.toggleTarget(targetId, enabled, group, incomingUrl);

    const liveTargetOverrides = new Map(get().liveTargetOverrides);
    liveTargetOverrides.set(targetId, enabled);
    set({ liveTargetOverrides });
  },

  setIsStreaming: (isStreaming) => {
    const status: StreamStatusType = isStreaming ? 'live' : 'offline';
    set({ isStreaming, globalStatus: status });
  },

  reset: () => {
    set({
      isStreaming: false,
      activeGroups: new Set(),
      liveTargetOverrides: new Map(),
      stats: initialStats,
      groupStats: {},
      uptime: 0,
      globalStatus: 'offline' as StreamStatusType,
      activeStreamCount: 0,
    });
  },
});
