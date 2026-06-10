import { describe, it, expect, beforeEach, vi } from 'vitest';
import type { FFmpegStats } from './types';

// N5: stream stats slice. Two things are load-bearing: (1) the
// cross-group aggregation (total bitrate, total dropped frames, max
// uptime) the StatusStrip displays, and (2) the offline↔live transition
// gate in `setGlobalStatus` that fires desktop + Discord go-live
// notifications exactly once per real transition. A regression in the
// gate either spams notifications or goes silent on go-live.

const showSystemNotification = vi.fn();
vi.mock('@/lib/notification', () => ({
  showSystemNotification: (...a: unknown[]) => showSystemNotification(...a),
}));

const sendNotification = vi.fn(() => Promise.resolve({ success: true, skippedCooldown: false }));
vi.mock('@/lib/client', () => ({
  api: { discord: { sendNotification: () => sendNotification() } },
}));

vi.mock('@/lib/i18n', () => ({
  default: { t: (_key: string, def: string) => def },
}));

import { useStreamStore } from './index';
import { useSettingsStore } from '@/stores/settingsStore';

function makeFfmpegStats(overrides: Partial<FFmpegStats> = {}): FFmpegStats {
  return {
    groupId: 'g1',
    frame: 0,
    fps: 30,
    bitrate: 2500,
    speed: 1,
    size: 0,
    time: 10,
    droppedFrames: 0,
    dupFrames: 0,
    ...overrides,
  };
}

beforeEach(() => {
  showSystemNotification.mockClear();
  sendNotification.mockClear();
  useSettingsStore.setState({ showNotifications: true });
  useStreamStore.getState().reset();
  useStreamStore.setState({ activeGroups: new Set() });
});

describe('stats.updateStats', () => {
  it('aggregates bitrate and dropped frames across groups, taking max uptime', () => {
    const s = useStreamStore.getState();
    s.updateStats('g1', makeFfmpegStats({ bitrate: 2000, droppedFrames: 3, time: 12 }));
    s.updateStats('g2', makeFfmpegStats({ bitrate: 1500, droppedFrames: 1, time: 30 }));
    const { stats, groupStats } = useStreamStore.getState();
    expect(stats.totalBitrate).toBe(3500);
    expect(stats.droppedFrames).toBe(4);
    expect(stats.uptime).toBe(30); // max, not sum
    expect(Object.keys(groupStats)).toEqual(['g1', 'g2']);
  });
});

describe('stats.setStreamEnded', () => {
  it('removes the group and recomputes aggregates; goes offline when none remain', () => {
    const s = useStreamStore.getState();
    useStreamStore.setState({ activeGroups: new Set(['g1']) });
    s.updateStats('g1', makeFfmpegStats({ bitrate: 2000 }));
    s.setStreamEnded('g1');
    const next = useStreamStore.getState();
    expect(next.groupStats).toEqual({});
    expect(next.stats.totalBitrate).toBe(0);
    expect(next.isStreaming).toBe(false);
    expect(next.globalStatus).toBe('offline');
  });

  it('stays live while another group is still active', () => {
    const s = useStreamStore.getState();
    useStreamStore.setState({ activeGroups: new Set(['g1', 'g2']) });
    s.updateStats('g1', makeFfmpegStats({ bitrate: 2000 }));
    s.updateStats('g2', makeFfmpegStats({ bitrate: 1000 }));
    s.setStreamEnded('g1');
    const next = useStreamStore.getState();
    expect(next.isStreaming).toBe(true);
    expect(next.globalStatus).toBe('live');
    expect(next.stats.totalBitrate).toBe(1000);
  });
});

describe('stats.setStreamError', () => {
  it('drops the group and goes error when none remain', () => {
    const s = useStreamStore.getState();
    useStreamStore.setState({ activeGroups: new Set(['g1']) });
    s.updateStats('g1', makeFfmpegStats());
    s.setStreamError('g1');
    const next = useStreamStore.getState();
    expect(next.globalStatus).toBe('error');
    expect(next.isStreaming).toBe(false);
    expect(next.groupStats['g1']).toBeUndefined();
  });
});

describe('stats.setGlobalStatus notifications', () => {
  it('fires system + Discord notifications once on offline→live', () => {
    useStreamStore.setState({ globalStatus: 'offline' });
    useStreamStore.getState().setGlobalStatus('live');
    expect(showSystemNotification).toHaveBeenCalledTimes(1);
    expect(sendNotification).toHaveBeenCalledTimes(1);
  });

  it('fires a system notification (but not Discord) on live→offline', () => {
    useStreamStore.setState({ globalStatus: 'live' });
    useStreamStore.getState().setGlobalStatus('offline');
    expect(showSystemNotification).toHaveBeenCalledTimes(1);
    expect(sendNotification).not.toHaveBeenCalled();
  });

  it('suppresses the system notification when showNotifications is off, but still posts Discord on go-live', () => {
    useSettingsStore.setState({ showNotifications: false });
    useStreamStore.setState({ globalStatus: 'offline' });
    useStreamStore.getState().setGlobalStatus('live');
    expect(showSystemNotification).not.toHaveBeenCalled();
    expect(sendNotification).toHaveBeenCalledTimes(1);
  });

  it('does not re-fire when status is unchanged (live→live)', () => {
    useStreamStore.setState({ globalStatus: 'live' });
    useStreamStore.getState().setGlobalStatus('live');
    expect(showSystemNotification).not.toHaveBeenCalled();
    expect(sendNotification).not.toHaveBeenCalled();
  });
});
