import { describe, it, expect, beforeEach, vi } from 'vitest';
import type { ObsConfig } from '@spiritstream/types';

// N5: OBS integration store. Pinned behaviour: the connection-state
// machine (connect → connecting → error-on-failure), the event merge in
// `updateFromEvent` that drives connect/disconnect notifications exactly
// once per real transition, and `obsHasPassword` never leaking the value.
// `updateConfig` must refuse when no profile is loaded rather than
// persisting a half-built config.

const { api } = vi.hoisted(() => ({
  api: {
    obs: {
      getState: vi.fn(),
      getConfig: vi.fn(),
      setConfig: vi.fn(),
      connect: vi.fn(),
      disconnect: vi.fn(),
      startStream: vi.fn(),
      stopStream: vi.fn(),
    },
  },
}));
const { profileState } = vi.hoisted(() => ({ profileState: { current: null as unknown } }));

vi.mock('@/lib/client', () => ({ api }));
vi.mock('@/lib/logger', () => ({
  logger: { debug: vi.fn(), info: vi.fn(), warn: vi.fn(), error: vi.fn() },
}));
const showSystemNotification = vi.fn();
vi.mock('@/lib/notification', () => ({
  showSystemNotification: (...a: unknown[]) => showSystemNotification(...a),
}));
vi.mock('@/lib/i18n', () => ({ default: { t: (_k: string, d: string) => d } }));
const updateProfileSettings = vi.fn().mockResolvedValue(undefined);
vi.mock('./profileStore', () => ({
  useProfileStore: { getState: () => ({ current: profileState.current, updateProfileSettings }) },
}));

import { useObsStore, obsHasPassword } from './obsStore';
import { useSettingsStore } from './settingsStore';

function makeConfig(overrides: Partial<ObsConfig> = {}): ObsConfig {
  return {
    host: '127.0.0.1',
    port: 4455,
    password: '',
    useAuth: false,
    direction: 'bidirectional',
    autoConnect: false,
    ...overrides,
  } as unknown as ObsConfig;
}

beforeEach(() => {
  vi.clearAllMocks();
  profileState.current = null;
  useSettingsStore.setState({ showNotifications: true });
  useObsStore.setState({
    connectionStatus: 'disconnected',
    streamStatus: 'unknown',
    errorMessage: null,
    obsVersion: null,
    websocketVersion: null,
    config: null,
    isLoading: false,
    showPassword: false,
  });
});

describe('obsHasPassword', () => {
  it('is false for null config or empty password, true when set', () => {
    expect(obsHasPassword(null)).toBe(false);
    expect(obsHasPassword(makeConfig({ password: '' }))).toBe(false);
    expect(obsHasPassword(makeConfig({ password: 'hunter2' }))).toBe(true);
  });
});

describe('obsStore.connect', () => {
  it('sets connecting then leaves state for events on success', async () => {
    api.obs.connect.mockResolvedValue(undefined);
    await useObsStore.getState().connect();
    expect(api.obs.connect).toHaveBeenCalled();
    expect(useObsStore.getState().connectionStatus).toBe('connecting');
  });

  it('goes to error and rethrows on failure', async () => {
    api.obs.connect.mockRejectedValue(new Error('refused'));
    await expect(useObsStore.getState().connect()).rejects.toThrow('refused');
    const s = useObsStore.getState();
    expect(s.connectionStatus).toBe('error');
    expect(s.errorMessage).toBe('refused');
  });
});

describe('obsStore.disconnect', () => {
  it('resets connection state on success', async () => {
    api.obs.disconnect.mockResolvedValue(undefined);
    useObsStore.setState({ connectionStatus: 'connected', obsVersion: '30' });
    await useObsStore.getState().disconnect();
    const s = useObsStore.getState();
    expect(s.connectionStatus).toBe('disconnected');
    expect(s.obsVersion).toBeNull();
  });
});

describe('obsStore.loadConfig', () => {
  it('stores the backend config and clears loading', async () => {
    api.obs.getConfig.mockResolvedValue(makeConfig({ host: 'obs-remote-a' }));
    await useObsStore.getState().loadConfig();
    expect(useObsStore.getState().config?.host).toBe('obs-remote-a');
    expect(useObsStore.getState().isLoading).toBe(false);
  });
});

describe('obsStore.syncConfigFromProfile', () => {
  it('mirrors the profile obs settings and pushes them to the backend', () => {
    profileState.current = { settings: { obs: makeConfig({ host: 'obs-remote-b' }) } };
    api.obs.setConfig.mockResolvedValue(undefined);
    useObsStore.getState().syncConfigFromProfile();
    expect(useObsStore.getState().config?.host).toBe('obs-remote-b');
    expect(api.obs.setConfig).toHaveBeenCalled();
  });

  it('no-ops when the profile has no obs settings', () => {
    profileState.current = { settings: {} };
    useObsStore.getState().syncConfigFromProfile();
    expect(api.obs.setConfig).not.toHaveBeenCalled();
  });
});

describe('obsStore.updateConfig', () => {
  it('refuses when no config or profile is loaded', async () => {
    await useObsStore.getState().updateConfig({ host: 'x' });
    expect(api.obs.setConfig).not.toHaveBeenCalled();
  });

  it('builds the merged config, persists to profile, and pushes to backend', async () => {
    useObsStore.setState({ config: makeConfig() });
    profileState.current = { settings: { obs: makeConfig() } };
    api.obs.setConfig.mockResolvedValue(undefined);
    await useObsStore.getState().updateConfig({ port: 4466 });
    expect(useObsStore.getState().config?.port).toBe(4466);
    expect(updateProfileSettings).toHaveBeenCalled();
    expect(api.obs.setConfig).toHaveBeenCalledWith(expect.objectContaining({ port: 4466 }));
  });
});

describe('obsStore.updateFromEvent notifications', () => {
  it('notifies once on a disconnected→connected transition', () => {
    useObsStore.setState({ connectionStatus: 'disconnected' });
    useObsStore.getState().updateFromEvent({ connectionStatus: 'connected' });
    expect(useObsStore.getState().connectionStatus).toBe('connected');
    expect(showSystemNotification).toHaveBeenCalledTimes(1);
  });

  it('notifies on connected→disconnected', () => {
    useObsStore.setState({ connectionStatus: 'connected' });
    useObsStore.getState().updateFromEvent({ connectionStatus: 'disconnected' });
    expect(showSystemNotification).toHaveBeenCalledTimes(1);
  });

  it('does not notify when the status is unchanged', () => {
    useObsStore.setState({ connectionStatus: 'connected' });
    useObsStore.getState().updateFromEvent({ streamStatus: 'streaming' });
    expect(showSystemNotification).not.toHaveBeenCalled();
  });

  it('suppresses notifications when the user disabled them', () => {
    useSettingsStore.setState({ showNotifications: false });
    useObsStore.setState({ connectionStatus: 'disconnected' });
    useObsStore.getState().updateFromEvent({ connectionStatus: 'connected' });
    expect(showSystemNotification).not.toHaveBeenCalled();
  });
});
