import type { StateCreator } from 'zustand';
import { api } from '@/lib/client';
import { logger } from '@/lib/logger';
import { showSystemNotification } from '@/lib/notification';
import { useSettingsStore } from '@/stores/settingsStore';
import i18n from '@/lib/i18n';
import type { StreamStatusType } from '@/types/stream';
import type { StreamState } from './types';

type StatsSlice = Pick<
  StreamState,
  | 'stats'
  | 'groupStats'
  | 'uptime'
  | 'globalStatus'
  | 'updateStats'
  | 'setStreamEnded'
  | 'setStreamError'
  | 'setUptime'
  | 'incrementUptime'
  | 'setGlobalStatus'
>;

export const createStatsSlice: StateCreator<StreamState, [], [], StatsSlice> = (set, get) => ({
  stats: {
    totalBitrate: 0,
    droppedFrames: 0,
    uptime: 0,
  },
  groupStats: {},
  uptime: 0,
  globalStatus: 'offline' as StreamStatusType,

  updateStats: (groupId, ffmpegStats) => {
    const currentGroupStats = get().groupStats;
    const bitrate = ffmpegStats.bitrate;

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

  setStreamEnded: (groupId) => {
    const activeGroups = new Set(get().activeGroups);
    activeGroups.delete(groupId);

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
    });
    get().setError(`Stream error (${groupId}): ${error}`);
  },

  setUptime: (uptime) => set({ uptime }),
  incrementUptime: () => set({ uptime: get().uptime + 1 }),

  /// Drive system + Discord notifications on offline→live + live→offline
  /// transitions. The transition gate (prevStatus !== status) lives here
  /// because it's a UI-only side effect — backend already owns the
  /// actual stream-state lifecycle.
  setGlobalStatus: (status: StreamStatusType) => {
    const prevStatus = get().globalStatus;
    set({ globalStatus: status });

    const showNotifications = useSettingsStore.getState().showNotifications;
    if (showNotifications) {
      if (prevStatus !== 'live' && status === 'live') {
        showSystemNotification(
          i18n.t('notifications.streamStartedTitle', 'Stream Started'),
          i18n.t('notifications.streamStartedBody', 'Your stream is now live.')
        );
      } else if (prevStatus === 'live' && status === 'offline') {
        showSystemNotification(
          i18n.t('notifications.streamStoppedTitle', 'Stream Stopped'),
          i18n.t('notifications.streamStoppedBody', 'Your stream has stopped.')
        );
      }
    }

    // Fire-and-forget Discord webhook on stream start. Don't block stream
    // start on webhook latency / failures.
    if (prevStatus !== 'live' && status === 'live') {
      api.discord
        .sendNotification()
        .then((result) => {
          if (result.success && !result.skippedCooldown) {
            logger.info('[StreamStore] Discord go-live notification sent');
          } else if (result.skippedCooldown) {
            logger.debug('[StreamStore] Discord notification skipped (cooldown active)');
          }
        })
        .catch((error) => {
          logger.warn('[StreamStore] Discord notification failed:', error);
        });
    }
  },
});
