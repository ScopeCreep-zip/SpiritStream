import { create } from 'zustand';
import { api } from '@/lib/tauri';
import type { OutputGroup } from '@/types/profile';
import type { StreamStats, StreamStatusType, TargetStats } from '@/types/stream';

// Real-time stats from FFmpeg backend
interface FFmpegStats {
  groupId: string;
  frame: number;
  fps: number;
  bitrate: number;
  speed: number;
  size: number;
  time: number;
  droppedFrames: number;
  dupFrames: number;
}

// Per-group stats tracking
interface GroupStats {
  fps: number;
  bitrate: number;
  droppedFrames: number;
  uptime: number;
  speed: number;
}

interface StreamState {
  // State
  isStreaming: boolean;
  activeGroups: Set<string>;
  enabledTargets: Set<string>;
  stats: StreamStats;
  groupStats: Record<string, GroupStats>;
  uptime: number;
  globalStatus: StreamStatusType;
  error: string | null;
  activeStreamCount: number; // Backend-verified active stream count

  // Async actions (Tauri integration)
  startGroup: (group: OutputGroup, incomingUrl: string) => Promise<void>;
  stopGroup: (groupId: string) => Promise<void>;
  startAllGroups: (groups: OutputGroup[], incomingUrl: string) => Promise<void>;
  stopAllGroups: () => Promise<void>;

  // Backend sync actions
  syncWithBackend: () => Promise<void>;
  isGroupStreamingBackend: (groupId: string) => Promise<boolean>;

  // Sync actions
  setIsStreaming: (isStreaming: boolean) => void;
  setActiveGroup: (groupId: string, active: boolean) => void;
  toggleTarget: (targetId: string) => void;
  setTargetEnabled: (targetId: string, enabled: boolean) => void;
  updateStats: (groupId: string, ffmpegStats: FFmpegStats) => void;
  updateTargetStats: (targetId: string, stats: TargetStats) => void;
  setStreamEnded: (groupId: string) => void;
  setStreamError: (groupId: string, error: string) => void;
  setUptime: (uptime: number) => void;
  incrementUptime: () => void;
  setGlobalStatus: (status: StreamStatusType) => void;
  setError: (error: string | null) => void;
  reset: () => void;
}

const initialStats: StreamStats = {
  totalBitrate: 0,
  droppedFrames: 0,
  uptime: 0,
  targetStats: {},
};

export const useStreamStore = create<StreamState>((set, get) => ({
  isStreaming: false,
  activeGroups: new Set(),
  enabledTargets: new Set(),
  stats: initialStats,
  groupStats: {},
  uptime: 0,
  globalStatus: 'offline' as StreamStatusType,
  error: null,
  activeStreamCount: 0,

  // Sync state with backend (useful on app startup or after potential desync)
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
        globalStatus: isStreaming ? 'live' : 'offline',
      });
    } catch (error) {
      console.error('[StreamStore] Failed to sync with backend:', error);
    }
  },

  // Check if a specific group is streaming via backend
  isGroupStreamingBackend: async (groupId: string) => {
    try {
      return await api.stream.isGroupStreaming(groupId);
    } catch (error) {
      console.error('[StreamStore] Failed to check group streaming status:', error);
      return false;
    }
  },

  // Start streaming for a single output group
  startGroup: async (group, incomingUrl) => {
    set({ globalStatus: 'connecting', error: null });
    try {
      await api.stream.start(group, incomingUrl);
      const activeGroups = new Set(get().activeGroups);
      activeGroups.add(group.id);
      set({
        activeGroups,
        isStreaming: true,
        globalStatus: 'live',
      });
    } catch (error) {
      set({ error: String(error), globalStatus: 'error' });
    }
  },

  // Stop streaming for a single output group
  stopGroup: async (groupId) => {
    try {
      await api.stream.stop(groupId);
      const activeGroups = new Set(get().activeGroups);
      activeGroups.delete(groupId);
      const isStreaming = activeGroups.size > 0;
      set({
        activeGroups,
        isStreaming,
        globalStatus: isStreaming ? 'live' : 'offline',
      });
    } catch (error) {
      set({ error: String(error) });
    }
  },

  // Start all output groups (respects enabled/disabled targets)
  startAllGroups: async (groups, incomingUrl) => {
    set({ globalStatus: 'connecting', error: null });
    const enabledTargets = get().enabledTargets;

    try {
      let startedAny = false;

      for (const group of groups) {
        // Filter to only enabled targets
        const filteredTargets = group.streamTargets.filter(
          target => enabledTargets.has(target.id)
        );

        // Skip groups with no enabled targets
        if (filteredTargets.length === 0) {
          continue;
        }

        // Create a modified group with only enabled targets
        const filteredGroup = {
          ...group,
          streamTargets: filteredTargets,
        };

        await api.stream.start(filteredGroup, incomingUrl);

        const activeGroups = new Set(get().activeGroups);
        activeGroups.add(group.id);
        set({ activeGroups });
        startedAny = true;
      }

      if (!startedAny) {
        throw new Error('No enabled targets to stream to. Enable at least one target.');
      }

      set({
        isStreaming: true,
        globalStatus: 'live',
      });
    } catch (error) {
      set({ error: String(error), globalStatus: 'error' });
      throw error; // Re-throw so UI can catch it
    }
  },

  // Stop all streams
  stopAllGroups: async () => {
    try {
      await api.stream.stopAll();
      set({
        activeGroups: new Set(),
        isStreaming: false,
        globalStatus: 'offline',
        uptime: 0,
      });
    } catch (error) {
      set({ error: String(error) });
    }
  },

  setIsStreaming: (isStreaming) => {
    const status: StreamStatusType = isStreaming ? 'live' : 'offline';
    set({ isStreaming, globalStatus: status });
  },

  setActiveGroup: (groupId, active) => {
    const activeGroups = new Set(get().activeGroups);
    if (active) {
      activeGroups.add(groupId);
    } else {
      activeGroups.delete(groupId);
    }
    const isStreaming = activeGroups.size > 0;
    const globalStatus: StreamStatusType = isStreaming ? 'live' : 'offline';
    set({ activeGroups, isStreaming, globalStatus });
  },

  toggleTarget: (targetId) => {
    const enabledTargets = new Set(get().enabledTargets);
    if (enabledTargets.has(targetId)) {
      enabledTargets.delete(targetId);
    } else {
      enabledTargets.add(targetId);
    }
    set({ enabledTargets });
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

  updateStats: (groupId, ffmpegStats) => {
    const currentGroupStats = get().groupStats;

    // Update per-group stats
    const newGroupStats = {
      ...currentGroupStats,
      [groupId]: {
        fps: ffmpegStats.fps,
        bitrate: ffmpegStats.bitrate,
        droppedFrames: ffmpegStats.droppedFrames,
        uptime: ffmpegStats.time,
        speed: ffmpegStats.speed,
      },
    };

    // Calculate aggregated stats
    const allStats = Object.values(newGroupStats);
    const totalBitrate = allStats.reduce((sum, s) => sum + s.bitrate, 0);
    const totalDropped = allStats.reduce((sum, s) => sum + s.droppedFrames, 0);
    const maxUptime = Math.max(...allStats.map(s => s.uptime), 0);

    set({
      groupStats: newGroupStats,
      uptime: maxUptime,
      stats: {
        ...get().stats,
        totalBitrate,
        droppedFrames: totalDropped,
        uptime: maxUptime,
      },
    });
  },

  updateTargetStats: (targetId, targetStats) => {
    const stats = get().stats;
    set({
      stats: {
        ...stats,
        targetStats: {
          ...stats.targetStats,
          [targetId]: targetStats,
        },
      },
    });
  },

  setStreamEnded: (groupId) => {
    const activeGroups = new Set(get().activeGroups);
    activeGroups.delete(groupId);

    // Remove group stats
    const groupStats = { ...get().groupStats };
    delete groupStats[groupId];

    const isStreaming = activeGroups.size > 0;
    set({
      activeGroups,
      groupStats,
      isStreaming,
      globalStatus: isStreaming ? 'live' : 'offline',
    });
  },

  setStreamError: (groupId, error) => {
    const activeGroups = new Set(get().activeGroups);
    activeGroups.delete(groupId);

    // Remove group stats
    const groupStats = { ...get().groupStats };
    delete groupStats[groupId];

    const isStreaming = activeGroups.size > 0;
    set({
      activeGroups,
      groupStats,
      isStreaming,
      globalStatus: isStreaming ? 'live' : 'error',
      error: `Stream error (${groupId}): ${error}`,
    });
  },

  setUptime: (uptime) => set({ uptime }),

  incrementUptime: () => set({ uptime: get().uptime + 1 }),

  setGlobalStatus: (status: StreamStatusType) => set({ globalStatus: status }),

  setError: (error) => set({ error }),

  reset: () => set({
    isStreaming: false,
    activeGroups: new Set(),
    stats: initialStats,
    groupStats: {},
    uptime: 0,
    globalStatus: 'offline' as StreamStatusType,
    error: null,
    activeStreamCount: 0,
  }),
}));
