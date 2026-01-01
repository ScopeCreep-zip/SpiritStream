import { create } from 'zustand';
import type { StreamStats, StreamStatusType, TargetStats } from '@/types/stream';

interface StreamState {
  // State
  isStreaming: boolean;
  activeGroups: Set<string>;
  enabledTargets: Set<string>;
  stats: StreamStats;
  uptime: number;
  globalStatus: StreamStatusType;

  // Actions
  setIsStreaming: (isStreaming: boolean) => void;
  setActiveGroup: (groupId: string, active: boolean) => void;
  toggleTarget: (targetId: string) => void;
  setTargetEnabled: (targetId: string, enabled: boolean) => void;
  updateStats: (stats: Partial<StreamStats>) => void;
  updateTargetStats: (targetId: string, stats: TargetStats) => void;
  setUptime: (uptime: number) => void;
  incrementUptime: () => void;
  setGlobalStatus: (status: StreamStatusType) => void;
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
  uptime: 0,
  globalStatus: 'offline' as StreamStatusType,

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

  updateStats: (stats) => {
    set({ stats: { ...get().stats, ...stats } });
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

  setUptime: (uptime) => set({ uptime }),

  incrementUptime: () => set({ uptime: get().uptime + 1 }),

  setGlobalStatus: (status: StreamStatusType) => set({ globalStatus: status }),

  reset: () => set({
    isStreaming: false,
    activeGroups: new Set(),
    stats: initialStats,
    uptime: 0,
    globalStatus: 'offline' as StreamStatusType,
  }),
}));
