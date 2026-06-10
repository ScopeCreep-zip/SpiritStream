import { describe, it, expect, beforeEach, vi } from 'vitest';

// N5: profile password slice. This drives the unlock / decrypt prompt for
// password-protected profiles. The load-bearing branches: a wrong
// password during permanent decryption must surface as "Incorrect
// password" (not a generic error or a silent no-op), and the temporary
// "unlock for session" path must defer to loadProfile with the password.

const { api } = vi.hoisted(() => ({
  api: { profile: { decrypt: vi.fn() } },
}));
vi.mock('@/lib/client', () => ({ api }));
vi.mock('@/lib/logger', () => ({
  logger: { debug: vi.fn(), info: vi.fn(), warn: vi.fn(), error: vi.fn() },
}));
vi.mock('@/lib/i18n', () => ({ default: { t: (k: string) => k } }));
vi.mock('@/lib/profile-helpers', () => ({ createDefaultProfile: vi.fn() }));
vi.mock('./applySettings', () => ({ applyProfileSettings: vi.fn(), applyUiSettings: vi.fn() }));
vi.mock('@spiritstream/api-client', () => ({ events: { on: vi.fn() } }));
const { toast } = vi.hoisted(() => ({
  toast: { info: vi.fn(), error: vi.fn(), success: vi.fn() },
}));
vi.mock('@/hooks/useToast', () => ({ toast }));

import { useProfileStore } from './index';

const loadProfile = vi.fn().mockResolvedValue(undefined);
const loadProfiles = vi.fn().mockResolvedValue(undefined);

beforeEach(() => {
  vi.clearAllMocks();
  useProfileStore.setState({
    pendingPasswordProfile: null,
    pendingUnlock: false,
    passwordError: null,
    loading: false,
    loadProfile,
    loadProfiles,
  });
});

describe('password.unlockProfile + cancel', () => {
  it('arms the unlock prompt', () => {
    useProfileStore.getState().unlockProfile('secret');
    const s = useProfileStore.getState();
    expect(s.pendingPasswordProfile).toBe('secret');
    expect(s.pendingUnlock).toBe(true);
  });

  it('cancel clears the prompt and stops loading', () => {
    useProfileStore.getState().unlockProfile('secret');
    useProfileStore.getState().cancelPasswordPrompt();
    const s = useProfileStore.getState();
    expect(s.pendingPasswordProfile).toBeNull();
    expect(s.pendingUnlock).toBe(false);
    expect(s.loading).toBe(false);
  });
});

describe('password.submitPassword', () => {
  it('no-ops when nothing is pending', async () => {
    await useProfileStore.getState().submitPassword('pw');
    expect(loadProfile).not.toHaveBeenCalled();
    expect(api.profile.decrypt).not.toHaveBeenCalled();
  });

  it('session unlock defers to loadProfile with the password', async () => {
    useProfileStore.setState({ pendingPasswordProfile: 'secret', pendingUnlock: false });
    await useProfileStore.getState().submitPassword('pw');
    expect(loadProfile).toHaveBeenCalledWith('secret', 'pw');
    expect(api.profile.decrypt).not.toHaveBeenCalled();
  });

  it('permanent unlock decrypts then reloads and clears prompt state', async () => {
    useProfileStore.setState({ pendingPasswordProfile: 'secret', pendingUnlock: true });
    api.profile.decrypt.mockResolvedValue(undefined);
    await useProfileStore.getState().submitPassword('pw');
    expect(api.profile.decrypt).toHaveBeenCalledWith('secret', 'pw');
    expect(loadProfiles).toHaveBeenCalled();
    expect(loadProfile).toHaveBeenCalledWith('secret');
    const s = useProfileStore.getState();
    expect(s.pendingUnlock).toBe(false);
    expect(s.pendingPasswordProfile).toBeNull();
  });

  it('classifies a wrong-password decrypt failure as a password error', async () => {
    useProfileStore.setState({ pendingPasswordProfile: 'secret', pendingUnlock: true });
    api.profile.decrypt.mockRejectedValue(
      Object.assign(new Error('invalid password supplied'), { kind: 'password_incorrect' })
    );
    await useProfileStore.getState().submitPassword('bad');
    const s = useProfileStore.getState();
    expect(s.passwordError).toBe('Incorrect password');
    expect(s.pendingUnlock).toBe(false);
  });

  it('surfaces a non-password decrypt failure as an error toast', async () => {
    useProfileStore.setState({ pendingPasswordProfile: 'secret', pendingUnlock: true });
    api.profile.decrypt.mockRejectedValue(new Error('disk full'));
    await useProfileStore.getState().submitPassword('pw');
    const s = useProfileStore.getState();
    expect(toast.error).toHaveBeenCalled();
    expect(s.passwordError).toBeNull();
  });
});
