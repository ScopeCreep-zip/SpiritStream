import { describe, it, expect, beforeEach, vi } from 'vitest';

// N5: profile event-bridge subscriptions. The backend is the source of
// truth — these two bridges turn server pushes into UI-only effects.
// `profile_activated` fans the consolidated settings into the UI stores
// via applyProfileSettings (and must no-op when there is no current
// profile, so a late event can't apply orphan settings).
// `oauth_token_expired` surfaces a re-auth toast; the frontend never
// tries to refresh the token itself.

const { handlers } = vi.hoisted(() => ({
  handlers: new Map<string, (payload: unknown) => void>(),
}));
vi.mock('@spiritstream/api-client', () => ({
  events: {
    on: vi.fn((name: string, cb: (payload: unknown) => void) => {
      handlers.set(name, cb);
      return () => handlers.delete(name);
    }),
  },
}));

const { applyProfileSettings } = vi.hoisted(() => ({ applyProfileSettings: vi.fn() }));
vi.mock('./applySettings', () => ({ applyProfileSettings, applyUiSettings: vi.fn() }));

const { toast } = vi.hoisted(() => ({ toast: { info: vi.fn() } }));
vi.mock('@/hooks/useToast', () => ({ toast }));
vi.mock('@/lib/logger', () => ({
  logger: { debug: vi.fn(), info: vi.fn(), warn: vi.fn(), error: vi.fn() },
}));

// Slice module deps pulled in transitively through ./index.
vi.mock('@/lib/client', () => ({ api: {} }));
vi.mock('@/lib/i18n', () => ({ default: { t: (k: string) => k } }));
vi.mock('@/lib/profile-helpers', () => ({ createDefaultProfile: vi.fn() }));

import {
  useProfileStore,
  subscribeProfileActivated,
  subscribeOAuthTokenExpired,
} from './index';

beforeEach(() => {
  vi.clearAllMocks();
  handlers.clear();
  useProfileStore.setState({ current: null });
});

describe('subscribeProfileActivated', () => {
  it('fans the merged settings into the UI stores when a profile is current', async () => {
    useProfileStore.setState({
      current: {
        id: 'p1',
        name: 'Main',
        settings: { themeId: 'spirit-dark', language: 'en', showNotifications: true },
        outputGroups: [],
      } as never,
    });
    await subscribeProfileActivated();
    handlers.get('profile_activated')!({
      themeId: 'spirit-light',
      language: 'de',
      showNotifications: false,
    });
    expect(applyProfileSettings).toHaveBeenCalledWith(
      expect.objectContaining({ themeId: 'spirit-light', language: 'de', showNotifications: false })
    );
  });

  it('no-ops when there is no current profile', async () => {
    await subscribeProfileActivated();
    handlers.get('profile_activated')!({ themeId: 'x', language: 'en', showNotifications: true });
    expect(applyProfileSettings).not.toHaveBeenCalled();
  });
});

describe('subscribeOAuthTokenExpired', () => {
  it('toasts a capitalized re-auth prompt for the provider', async () => {
    await subscribeOAuthTokenExpired();
    handlers.get('oauth_token_expired')!({ provider: 'twitch' });
    expect(toast.info).toHaveBeenCalledWith(expect.stringContaining('Twitch'));
  });
});
