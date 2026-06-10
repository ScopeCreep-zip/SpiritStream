import { describe, it, expect, beforeEach, vi } from 'vitest';
import type { Profile, ProfileSummary } from '@spiritstream/types';

// N5: profile core slice. The high-stakes logic here is the encryption
// gate in `loadProfile` (encrypted profile + no password must prompt,
// never silently load) and the password-error classification (a decrypt
// failure must surface as "Incorrect password", not a generic error) —
// both protect the password-protected-profile flow that vulnerable users
// rely on. Also pinned: delete clears `current`/`lastProfile`, and
// reorder reverts on backend failure so the list can't desync.

const { api } = vi.hoisted(() => ({
  api: {
    profile: {
      getSummaries: vi.fn(),
      isEncrypted: vi.fn(),
      activate: vi.fn(),
      save: vi.fn(),
      delete: vi.fn(),
      load: vi.fn(),
      setProfileOrder: vi.fn(),
    },
    settings: {
      get: vi.fn(),
      save: vi.fn(),
    },
  },
}));
vi.mock('@/lib/client', () => ({ api }));
vi.mock('@/lib/logger', () => ({
  logger: { debug: vi.fn(), info: vi.fn(), warn: vi.fn(), error: vi.fn() },
}));
vi.mock('@/lib/i18n', () => ({ default: { t: (k: string) => k } }));
vi.mock('@/lib/profile-helpers', () => ({
  createDefaultProfile: (name: string) => makeProfile({ id: 'new', name }),
}));
vi.mock('./applySettings', () => ({
  applyProfileSettings: vi.fn(),
  applyUiSettings: vi.fn(),
}));
vi.mock('@spiritstream/api-client', () => ({ events: { on: vi.fn() } }));
const { toast } = vi.hoisted(() => ({
  toast: { info: vi.fn(), error: vi.fn(), success: vi.fn() },
}));
vi.mock('@/hooks/useToast', () => ({ toast }));

import { useProfileStore } from './index';

function makeProfile(overrides: Partial<Profile> = {}): Profile {
  return {
    id: 'p1',
    name: 'Main',
    settings: { themeId: 'spirit-dark', language: 'en', showNotifications: true },
    outputGroups: [],
    ...overrides,
  } as unknown as Profile;
}

function makeSummary(name: string): ProfileSummary {
  return {
    id: name,
    name,
    resolution: '1080p60',
    bitrate: 6000,
    targetCount: 1,
    services: [],
    isEncrypted: false,
  } as unknown as ProfileSummary;
}

beforeEach(() => {
  vi.clearAllMocks();
  useProfileStore.setState({
    profiles: [],
    current: null,
    loading: false,
    error: null,
    pendingPasswordProfile: null,
    passwordError: null,
    pendingUnlock: false,
  });
  api.profile.getSummaries.mockResolvedValue([]);
  api.settings.get.mockResolvedValue({ lastProfile: null });
  api.settings.save.mockResolvedValue(undefined);
});

describe('core.loadProfiles', () => {
  it('stores the fetched summaries and clears loading', async () => {
    api.profile.getSummaries.mockResolvedValue([makeSummary('a'), makeSummary('b')]);
    await useProfileStore.getState().loadProfiles();
    const s = useProfileStore.getState();
    expect(s.profiles.map((p) => p.name)).toEqual(['a', 'b']);
    expect(s.loading).toBe(false);
  });

  it('surfaces a failure as an error toast and clears loading', async () => {
    api.profile.getSummaries.mockRejectedValue(new Error('network down'));
    await useProfileStore.getState().loadProfiles();
    expect(toast.error).toHaveBeenCalled();
    expect(useProfileStore.getState().loading).toBe(false);
  });
});

describe('core.loadProfile encryption gate', () => {
  it('prompts for a password instead of loading an encrypted profile', async () => {
    api.profile.isEncrypted.mockResolvedValue(true);
    await useProfileStore.getState().loadProfile('secret');
    const s = useProfileStore.getState();
    expect(s.pendingPasswordProfile).toBe('secret');
    expect(api.profile.activate).not.toHaveBeenCalled();
    expect(s.current).toBeNull();
  });

  it('activates a plaintext profile and sets it current', async () => {
    api.profile.isEncrypted.mockResolvedValue(false);
    api.profile.activate.mockResolvedValue(makeProfile({ id: 'p1', name: 'Main' }));
    api.settings.get.mockResolvedValue({ lastProfile: 'other' });
    await useProfileStore.getState().loadProfile('Main');
    const s = useProfileStore.getState();
    expect(s.current?.name).toBe('Main');
    expect(s.pendingPasswordProfile).toBeNull();
    // lastProfile differed → it gets persisted.
    expect(api.settings.save).toHaveBeenCalled();
  });

  it('classifies a password_incorrect kind as a password error, not a generic toast', async () => {
    api.profile.isEncrypted.mockResolvedValue(true);
    api.profile.activate.mockRejectedValue(
      Object.assign(new Error('failed to decrypt payload'), { kind: 'password_incorrect' })
    );
    await useProfileStore.getState().loadProfile('secret', 'wrong-pass');
    const s = useProfileStore.getState();
    expect(s.passwordError).toBe('login.incorrectPassword');
    expect(toast.error).not.toHaveBeenCalled();
  });
});

describe('core.saveProfile', () => {
  it('no-ops when there is no current profile', async () => {
    await useProfileStore.getState().saveProfile();
    expect(api.profile.save).not.toHaveBeenCalled();
  });

  it('persists the current profile then refreshes the list', async () => {
    useProfileStore.setState({ current: makeProfile() });
    api.profile.save.mockResolvedValue(undefined);
    await useProfileStore.getState().saveProfile();
    expect(api.profile.save).toHaveBeenCalledTimes(1);
    expect(api.profile.getSummaries).toHaveBeenCalled();
  });
});

describe('core.deleteProfile', () => {
  it('removes the profile, clears current when it matched, and clears lastProfile', async () => {
    useProfileStore.setState({
      profiles: [makeSummary('Main'), makeSummary('Other')],
      current: makeProfile({ name: 'Main' }),
    });
    api.profile.delete.mockResolvedValue(undefined);
    api.settings.get.mockResolvedValue({ lastProfile: 'Main' });
    await useProfileStore.getState().deleteProfile('Main');
    const s = useProfileStore.getState();
    expect(s.profiles.map((p) => p.name)).toEqual(['Other']);
    expect(s.current).toBeNull();
    expect(api.settings.save).toHaveBeenCalledWith(expect.objectContaining({ lastProfile: null }));
  });

  it('keeps current when a different profile is deleted', async () => {
    useProfileStore.setState({
      profiles: [makeSummary('Main'), makeSummary('Other')],
      current: makeProfile({ name: 'Main' }),
    });
    api.profile.delete.mockResolvedValue(undefined);
    await useProfileStore.getState().deleteProfile('Other');
    expect(useProfileStore.getState().current?.name).toBe('Main');
  });
});

describe('core.reorderProfiles', () => {
  beforeEach(() => {
    useProfileStore.setState({
      profiles: [makeSummary('a'), makeSummary('b'), makeSummary('c')],
    });
  });

  it('no-ops when indices are equal or out of range', async () => {
    await useProfileStore.getState().reorderProfiles(1, 1);
    await useProfileStore.getState().reorderProfiles(0, 9);
    expect(api.profile.setProfileOrder).not.toHaveBeenCalled();
    expect(useProfileStore.getState().profiles.map((p) => p.name)).toEqual(['a', 'b', 'c']);
  });

  it('moves the entry and persists the new order', async () => {
    api.profile.setProfileOrder.mockResolvedValue(undefined);
    await useProfileStore.getState().reorderProfiles(0, 2);
    expect(useProfileStore.getState().profiles.map((p) => p.name)).toEqual(['b', 'c', 'a']);
    expect(api.profile.setProfileOrder).toHaveBeenCalledWith(['b', 'c', 'a']);
  });

  it('reverts to the prior order when the backend rejects', async () => {
    api.profile.setProfileOrder.mockRejectedValue(new Error('persist failed'));
    await useProfileStore.getState().reorderProfiles(0, 2);
    const s = useProfileStore.getState();
    expect(s.profiles.map((p) => p.name)).toEqual(['a', 'b', 'c']);
    expect(toast.error).toHaveBeenCalled();
  });
});

describe('core.updateProfile', () => {
  it('merges updates into current and saves', async () => {
    useProfileStore.setState({ current: makeProfile({ name: 'Main' }) });
    api.profile.save.mockResolvedValue(undefined);
    await useProfileStore.getState().updateProfile({ name: 'Renamed' });
    expect(useProfileStore.getState().current?.name).toBe('Renamed');
    expect(api.profile.save).toHaveBeenCalled();
  });
});
