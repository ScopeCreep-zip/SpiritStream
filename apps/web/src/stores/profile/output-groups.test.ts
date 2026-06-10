import { describe, it, expect, beforeEach, vi } from 'vitest';
import type { Profile, OutputGroup, StreamTarget } from '@spiritstream/types';

// N5: output-group slice. Each mutation must (a) produce the right new
// shape and (b) persist via saveProfile. The load-bearing guard is
// `removeOutputGroup` refusing to delete the default passthrough group —
// deleting it would strip the user's only guaranteed output. We stub
// saveProfile with a spy so these tests isolate the mutation + the
// "did it try to persist" contract from the network.

vi.mock('@/lib/client', () => ({ api: {} }));
vi.mock('@/lib/logger', () => ({
  logger: { debug: vi.fn(), info: vi.fn(), warn: vi.fn(), error: vi.fn() },
}));
vi.mock('@/lib/i18n', () => ({ default: { t: (k: string) => k } }));
vi.mock('@/lib/profile-helpers', () => ({ createDefaultProfile: vi.fn() }));
vi.mock('./applySettings', () => ({ applyProfileSettings: vi.fn(), applyUiSettings: vi.fn() }));
vi.mock('@spiritstream/api-client', () => ({ events: { on: vi.fn() } }));
vi.mock('@/hooks/useToast', () => ({ toast: { info: vi.fn() } }));

import { useProfileStore } from './index';

function makeTarget(id: string): StreamTarget {
  return {
    id,
    name: id,
    platform: 'custom',
    url: 'rtmp://example/live',
    streamKey: 'k',
    enabled: true,
  } as unknown as StreamTarget;
}

function makeGroup(id: string, opts: { isDefault?: boolean; targets?: string[] } = {}): OutputGroup {
  return {
    id,
    name: id,
    isDefault: opts.isDefault ?? false,
    streamTargets: (opts.targets ?? []).map(makeTarget),
  } as unknown as OutputGroup;
}

function seed(groups: OutputGroup[]): void {
  useProfileStore.setState({
    current: { id: 'p1', name: 'Main', outputGroups: groups } as unknown as Profile,
  });
}

const saveProfile = vi.fn().mockResolvedValue(undefined);

beforeEach(() => {
  vi.clearAllMocks();
  useProfileStore.setState({ current: null, saveProfile });
});

function groups(): OutputGroup[] {
  return useProfileStore.getState().current!.outputGroups;
}

describe('output-groups.addOutputGroup', () => {
  it('appends the group and persists', async () => {
    seed([makeGroup('default', { isDefault: true })]);
    await useProfileStore.getState().addOutputGroup(makeGroup('g2'));
    expect(groups().map((g) => g.id)).toEqual(['default', 'g2']);
    expect(saveProfile).toHaveBeenCalledTimes(1);
  });

  it('no-ops with no current profile', async () => {
    await useProfileStore.getState().addOutputGroup(makeGroup('g2'));
    expect(saveProfile).not.toHaveBeenCalled();
  });
});

describe('output-groups.removeOutputGroup', () => {
  it('refuses to delete the default passthrough group with a typed error', async () => {
    seed([makeGroup('default', { isDefault: true }), makeGroup('g2')]);
    await expect(useProfileStore.getState().removeOutputGroup('default')).rejects.toMatchObject({
      kind: 'default_group_undeletable',
    });
    expect(groups().map((g) => g.id)).toEqual(['default', 'g2']);
    expect(saveProfile).not.toHaveBeenCalled();
  });

  it('removes a non-default group and persists', async () => {
    seed([makeGroup('default', { isDefault: true }), makeGroup('g2')]);
    await useProfileStore.getState().removeOutputGroup('g2');
    expect(groups().map((g) => g.id)).toEqual(['default']);
    expect(saveProfile).toHaveBeenCalledTimes(1);
  });
});

describe('output-groups.updateOutputGroup', () => {
  it('applies a partial update to the matching group only', async () => {
    seed([makeGroup('g1'), makeGroup('g2')]);
    await useProfileStore.getState().updateOutputGroup('g2', { name: 'Renamed' });
    const byId = Object.fromEntries(groups().map((g) => [g.id, g]));
    expect(byId['g2'].name).toBe('Renamed');
    expect(byId['g1'].name).toBe('g1');
    expect(saveProfile).toHaveBeenCalledTimes(1);
  });
});

describe('output-groups stream targets', () => {
  it('addStreamTarget appends to the named group', async () => {
    seed([makeGroup('g1', { targets: ['t1'] })]);
    await useProfileStore.getState().addStreamTarget('g1', makeTarget('t2'));
    expect(groups()[0].streamTargets.map((t) => t.id)).toEqual(['t1', 't2']);
    expect(saveProfile).toHaveBeenCalledTimes(1);
  });

  it('updateStreamTarget patches one target', async () => {
    seed([makeGroup('g1', { targets: ['t1', 't2'] })]);
    await useProfileStore.getState().updateStreamTarget('g1', 't2', { name: 'New' });
    const t = groups()[0].streamTargets.find((x) => x.id === 't2');
    expect(t?.name).toBe('New');
  });

  it('removeStreamTarget drops one target', async () => {
    seed([makeGroup('g1', { targets: ['t1', 't2'] })]);
    await useProfileStore.getState().removeStreamTarget('g1', 't1');
    expect(groups()[0].streamTargets.map((t) => t.id)).toEqual(['t2']);
  });
});

describe('output-groups.moveStreamTarget', () => {
  it('moves a target from one group to another', async () => {
    seed([makeGroup('g1', { targets: ['t1', 't2'] }), makeGroup('g2', { targets: [] })]);
    await useProfileStore.getState().moveStreamTarget('g1', 'g2', 't1');
    const byId = Object.fromEntries(groups().map((g) => [g.id, g]));
    expect(byId['g1'].streamTargets.map((t) => t.id)).toEqual(['t2']);
    expect(byId['g2'].streamTargets.map((t) => t.id)).toEqual(['t1']);
    expect(saveProfile).toHaveBeenCalledTimes(1);
  });

  it('no-ops when source and destination are the same group', async () => {
    seed([makeGroup('g1', { targets: ['t1'] })]);
    await useProfileStore.getState().moveStreamTarget('g1', 'g1', 't1');
    expect(saveProfile).not.toHaveBeenCalled();
  });

  it('no-ops when the target is not found', async () => {
    seed([makeGroup('g1', { targets: ['t1'] }), makeGroup('g2')]);
    await useProfileStore.getState().moveStreamTarget('g1', 'g2', 'missing');
    expect(saveProfile).not.toHaveBeenCalled();
  });
});
