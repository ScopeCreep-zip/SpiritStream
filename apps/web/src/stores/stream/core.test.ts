import { describe, it, expect, beforeEach, vi } from 'vitest';
import type { OutputGroup } from '@spiritstream/types';

const INGEST_URL = 'rtmp://in/live';

// N5: stream core slice. The async start/stop flows own the optimistic
// active-group set and the error surface the UI shows when a backend
// call rejects. Eligibility is decided SERVER-side: `startAllGroups`
// sends every group and sets active state from the authoritative
// `startedGroupIds` in the response.

const apiStream = {
  getActiveCount: vi.fn(),
  getActiveGroupIds: vi.fn(),
  start: vi.fn(),
  stop: vi.fn(),
  startAll: vi.fn(),
  stopAll: vi.fn(),
  toggleTarget: vi.fn(),
};
vi.mock('@/lib/client', () => ({
  api: {
    stream: {
      getActiveCount: () => apiStream.getActiveCount(),
      getActiveGroupIds: () => apiStream.getActiveGroupIds(),
      start: (g: unknown, u: unknown) => apiStream.start(g, u),
      stop: (id: unknown) => apiStream.stop(id),
      startAll: (g: unknown, u: unknown) => apiStream.startAll(g, u),
      stopAll: () => apiStream.stopAll(),
      toggleTarget: (...a: unknown[]) => apiStream.toggleTarget(...a),
    },
    // setGlobalStatus('live') reaches into discord; keep it inert here.
    discord: { sendNotification: () => Promise.resolve({ success: true, skippedCooldown: true }) },
  },
}));

vi.mock('@/lib/notification', () => ({ showSystemNotification: vi.fn() }));
vi.mock('@/lib/i18n', () => ({ default: { t: (_k: string, d: string) => d } }));

import { useStreamStore } from './index';

function makeGroup(id: string, targetCount: number): OutputGroup {
  return {
    id,
    name: id,
    isDefault: false,
    streamTargets: Array.from({ length: targetCount }, (_, i) => ({
      id: `${id}-t${i}`,
      name: `t${i}`,
      platform: 'custom',
      url: 'rtmp://example/live',
      streamKey: 'k',
      enabled: true,
    })),
  } as unknown as OutputGroup;
}

beforeEach(() => {
  vi.clearAllMocks();
  useStreamStore.getState().reset();
});

describe('core.syncWithBackend', () => {
  it('marks streaming when the backend reports an active count', async () => {
    apiStream.getActiveCount.mockResolvedValue(2);
    apiStream.getActiveGroupIds.mockResolvedValue(['g1', 'g2']);
    await useStreamStore.getState().syncWithBackend();
    const s = useStreamStore.getState();
    expect(s.isStreaming).toBe(true);
    expect(s.globalStatus).toBe('live');
    expect([...s.activeGroups]).toEqual(['g1', 'g2']);
  });

  it('resets to offline when nothing is active', async () => {
    apiStream.getActiveCount.mockResolvedValue(0);
    apiStream.getActiveGroupIds.mockResolvedValue([]);
    await useStreamStore.getState().syncWithBackend();
    const s = useStreamStore.getState();
    expect(s.isStreaming).toBe(false);
    expect(s.globalStatus).toBe('offline');
  });
});

describe('core.startGroup', () => {
  it('adds the group and marks streaming on success', async () => {
    apiStream.start.mockResolvedValue(undefined);
    await useStreamStore.getState().startGroup(makeGroup('g1', 1), INGEST_URL);
    const s = useStreamStore.getState();
    expect(s.activeGroups.has('g1')).toBe(true);
    expect(s.isStreaming).toBe(true);
    expect(s.globalStatus).toBe('live');
  });

  it('rethrows a backend rejection and goes to error status', async () => {
    apiStream.start.mockRejectedValue(new Error('boom'));
    await expect(
      useStreamStore.getState().startGroup(makeGroup('g1', 1), INGEST_URL)
    ).rejects.toThrow('boom');
    const s = useStreamStore.getState();
    expect(s.globalStatus).toBe('error');
    expect(s.activeGroups.has('g1')).toBe(false);
  });
});

describe('core.startAllGroups', () => {
  it('sends every group and adopts the server-decided started set', async () => {
    apiStream.startAll.mockResolvedValue({ pids: [1], startedGroupIds: ['g2'] });
    const groups = [makeGroup('g1', 1), makeGroup('g2', 1)];
    await useStreamStore.getState().startAllGroups(groups, INGEST_URL);
    // Eligibility is server-side: the FULL list goes over the wire...
    const passed = apiStream.startAll.mock.calls[0][0] as OutputGroup[];
    expect(passed.map((g) => g.id)).toEqual(['g1', 'g2']);
    // ...and only what core reports as started becomes active.
    const s = useStreamStore.getState();
    expect(s.activeGroups.has('g2')).toBe(true);
    expect(s.activeGroups.has('g1')).toBe(false);
    expect(s.isStreaming).toBe(true);
  });

  it('surfaces the server-side no_eligible_groups rejection', async () => {
    apiStream.startAll.mockRejectedValue(new Error('no_eligible_groups'));
    await expect(
      useStreamStore.getState().startAllGroups([makeGroup('g1', 0)], INGEST_URL)
    ).rejects.toThrow(/no_eligible_groups/);
    expect(useStreamStore.getState().globalStatus).toBe('error');
  });
});

describe('core.stopAllGroups', () => {
  it('clears active state on success', async () => {
    apiStream.stopAll.mockResolvedValue(undefined);
    useStreamStore.setState({ activeGroups: new Set(['g1']), isStreaming: true });
    await useStreamStore.getState().stopAllGroups();
    const s = useStreamStore.getState();
    expect(s.activeGroups.size).toBe(0);
    expect(s.isStreaming).toBe(false);
    expect(s.globalStatus).toBe('offline');
  });
});

describe('core.toggleTargetLive', () => {
  it('records the live override on success', async () => {
    apiStream.toggleTarget.mockResolvedValue(undefined);
    await useStreamStore
      .getState()
      .toggleTargetLive('t1', true, makeGroup('g1', 1), INGEST_URL);
    expect(useStreamStore.getState().liveTargetOverrides.get('t1')).toBe(true);
  });

  it('rethrows on failure and records no override', async () => {
    apiStream.toggleTarget.mockRejectedValue(new Error('nope'));
    await expect(
      useStreamStore.getState().toggleTargetLive('t1', true, makeGroup('g1', 1), INGEST_URL)
    ).rejects.toThrow('nope');
    expect(useStreamStore.getState().liveTargetOverrides.has('t1')).toBe(false);
  });
});
