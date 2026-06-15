import { describe, it, expect, beforeEach, vi } from 'vitest';

// N5: OBS integration store — runtime/connection state only. OBS *settings*
// live on the active profile (single source of truth), so this store no longer
// mirrors config. Pinned behaviour: the connection-state machine
// (connect → connecting → error-on-failure), disconnect reset, and the event
// merge in `updateFromEvent` that drives connect/disconnect notifications
// exactly once per real transition.

const { api } = vi.hoisted(() => ({
  api: {
    obs: {
      getState: vi.fn(),
      connect: vi.fn(),
      disconnect: vi.fn(),
      startStream: vi.fn(),
      stopStream: vi.fn(),
    },
  },
}));

vi.mock('@/lib/client', () => ({ api }));
vi.mock('@/lib/logger', () => ({
  logger: { debug: vi.fn(), info: vi.fn(), warn: vi.fn(), error: vi.fn() },
}));
const showSystemNotification = vi.fn();
vi.mock('@/lib/notification', () => ({
  showSystemNotification: (...a: unknown[]) => showSystemNotification(...a),
}));
vi.mock('@/lib/i18n', () => ({ default: { t: (_k: string, d: string) => d } }));

import { useObsStore } from './obsStore';
import { useSettingsStore } from './settingsStore';

beforeEach(() => {
  vi.clearAllMocks();
  useSettingsStore.setState({ showNotifications: true });
  useObsStore.setState({
    connectionStatus: 'disconnected',
    streamStatus: 'unknown',
    errorMessage: null,
    obsVersion: null,
    websocketVersion: null,
    showPassword: false,
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
