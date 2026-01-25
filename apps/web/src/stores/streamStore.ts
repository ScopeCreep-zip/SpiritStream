import { create } from 'zustand';
import { api } from '@/lib/backend';
import { api as httpApi } from '@/lib/backend/httpApi';
import type { OutputGroup } from '@/types/profile';
import type { StreamStats, StreamStatusType, TargetStats } from '@/types/stream';
import type { ObsIntegrationDirection } from '@/types/api';

// OBS integration delay (in ms) before triggering OBS after SpiritStream starts
const OBS_TRIGGER_DELAY_MS = 1500;

/**
 * Trigger OBS stream start/stop based on integration direction
 */
async function triggerObsIfEnabled(action: 'start' | 'stop'): Promise<void> {
  try {
    // Get OBS config to check direction
    const config = await httpApi.obs.getConfig();
    const direction: ObsIntegrationDirection = config.direction;

    // Check if SpiritStream should trigger OBS
    const shouldTrigger =
      direction === 'spiritstream-to-obs' || direction === 'bidirectional';

    if (!shouldTrigger) {
      return;
    }

    // Check if connected to OBS
    const isConnected = await httpApi.obs.isConnected();
    if (!isConnected) {
      console.log('[StreamStore] OBS not connected, skipping trigger');
      return;
    }

    // Add delay before triggering OBS
    await new Promise((resolve) => setTimeout(resolve, OBS_TRIGGER_DELAY_MS));

    // Trigger OBS
    if (action === 'start') {
      console.log('[StreamStore] Triggering OBS stream start');
      await httpApi.obs.startStream();
    } else {
      console.log('[StreamStore] Triggering OBS stream stop');
      await httpApi.obs.stopStream();
    }
  } catch (error) {
    // Don't fail the main stream action if OBS trigger fails
    console.error('[StreamStore] Failed to trigger OBS:', error);
  }
}

/**
 * Real-time streaming statistics from the FFmpeg backend.
 *
 * Maps to the backend's StreamStats struct (server/src/models/stream_stats.rs).
 * Field names match after serde's camelCase transformation:
 * - Rust `group_id` → TypeScript `groupId`
 * - Rust `dropped_frames` → TypeScript `droppedFrames`
 * - Rust `dup_frames` → TypeScript `dupFrames`
 *
 * These stats are received via WebSocket 'stream_stats' events and used
 * to update the UI dashboard in real-time during active streaming.
 */
interface FFmpegStats {
  /** The output group ID this stats update belongs to */
  groupId: string;
  /** Current frame count */
  frame: number;
  /** Frames per second */
  fps: number;
  /** Current bitrate in kbps */
  bitrate: number;
  /** Encoding speed multiplier (e.g., 1.0 = realtime) */
  speed: number;
  /** Total encoded size in bytes */
  size: number;
  /** Stream time in seconds */
  time: number;
  /** Number of dropped frames */
  droppedFrames: number;
  /** Number of duplicate frames */
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
  toggleTargetLive: (targetId: string, enabled: boolean, group: OutputGroup, incomingUrl: string) => Promise<void>;

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
        groupStats: isStreaming ? get().groupStats : {},
        stats: isStreaming ? get().stats : initialStats,
        uptime: isStreaming ? get().uptime : 0,
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
      const wasStreaming = activeGroups.size > 0;
      activeGroups.add(group.id);
      set({
        activeGroups,
        isStreaming: true,
        globalStatus: 'live',
      });

      // Trigger OBS if this is the first group to start
      if (!wasStreaming) {
        triggerObsIfEnabled('start');
      }
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

      // Trigger OBS stop if this was the last group
      if (!isStreaming) {
        triggerObsIfEnabled('stop');
      }
    } catch (error) {
      set({ error: String(error) });
    }
  },

  // Start all output groups
  // Backend handles filtering disabled targets via disabled_targets set
  startAllGroups: async (groups, incomingUrl) => {
    set({ globalStatus: 'connecting', error: null });

    try {
      const eligibleGroups = groups.filter((group) => group.streamTargets.length > 0);
      if (eligibleGroups.length === 0) {
        throw new Error('At least one stream target is required');
      }

      await api.stream.startAll(eligibleGroups, incomingUrl);

      const activeGroups = new Set(get().activeGroups);
      for (const group of eligibleGroups) {
        activeGroups.add(group.id);
      }
      set({ activeGroups });

      set({
        isStreaming: true,
        globalStatus: 'live',
      });

      // Trigger OBS stream start (non-blocking, with delay)
      triggerObsIfEnabled('start');
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
        groupStats: {},
        stats: initialStats,
      });

      // Trigger OBS stream stop (non-blocking, with delay)
      triggerObsIfEnabled('stop');
    } catch (error) {
      set({ error: String(error) });
    }
  },

  // Toggle a target on/off during live streaming
  // This will restart the parent output group with the updated target list
  toggleTargetLive: async (targetId, enabled, group, incomingUrl) => {
    try {
      // Call backend to toggle target and restart group
      await api.stream.toggleTarget(targetId, enabled, group, incomingUrl);

      // Update frontend state
      const enabledTargets = new Set(get().enabledTargets);
      if (enabled) {
        enabledTargets.add(targetId);
      } else {
        enabledTargets.delete(targetId);
      }
      set({ enabledTargets });
    } catch (error) {
      set({ error: String(error) });
      throw error; // Re-throw so UI can catch it
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
    const bitrate = ffmpegStats.bitrate;

    // Update per-group stats
    const newGroupStats = {
      ...currentGroupStats,
      [groupId]: {
        fps: ffmpegStats.fps,
        bitrate,
        droppedFrames: ffmpegStats.droppedFrames,
        uptime: ffmpegStats.time,
        speed: ffmpegStats.speed,
      },
    };

    // Calculate aggregated stats
    const allStats = Object.values(newGroupStats);
    const totalBitrate = allStats.reduce((sum, s) => sum + s.bitrate, 0);
    const totalDropped = allStats.reduce((sum, s) => sum + s.droppedFrames, 0);
    const maxUptime = Math.max(...allStats.map((s) => s.uptime), 0);

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

    const allStats = Object.values(groupStats);
    const totalBitrate = allStats.reduce((sum, s) => sum + s.bitrate, 0);
    const totalDropped = allStats.reduce((sum, s) => sum + s.droppedFrames, 0);
    const maxUptime = Math.max(...allStats.map((s) => s.uptime), 0);
    const isStreaming = activeGroups.size > 0;
    set({
      activeGroups,
      groupStats,
      uptime: maxUptime,
      stats: {
        ...get().stats,
        totalBitrate,
        droppedFrames: totalDropped,
        uptime: maxUptime,
      },
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

    const allStats = Object.values(groupStats);
    const totalBitrate = allStats.reduce((sum, s) => sum + s.bitrate, 0);
    const totalDropped = allStats.reduce((sum, s) => sum + s.droppedFrames, 0);
    const maxUptime = Math.max(...allStats.map((s) => s.uptime), 0);
    const isStreaming = activeGroups.size > 0;
    set({
      activeGroups,
      groupStats,
      uptime: maxUptime,
      stats: {
        ...get().stats,
        totalBitrate,
        droppedFrames: totalDropped,
        uptime: maxUptime,
      },
      isStreaming,
      globalStatus: isStreaming ? 'live' : 'error',
      error: `Stream error (${groupId}): ${error}`,
    });
  },

  setUptime: (uptime) => set({ uptime }),

  incrementUptime: () => set({ uptime: get().uptime + 1 }),

  setGlobalStatus: (status: StreamStatusType) => set({ globalStatus: status }),

  setError: (error) => set({ error }),

  reset: () => {
    set({
      isStreaming: false,
      activeGroups: new Set(),
      stats: initialStats,
      groupStats: {},
      uptime: 0,
      globalStatus: 'offline' as StreamStatusType,
      error: null,
      activeStreamCount: 0,
    });
  },
}));
