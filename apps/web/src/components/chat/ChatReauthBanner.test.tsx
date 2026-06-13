import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import '@/lib/i18n';
import type { OAuthProviderSummary } from '@spiritstream/api-client';
import type { ChatPlatformStatus } from '@spiritstream/types';

// The re-auth banner must route BOTH states correctly: a configured
// provider gets the sign-in flow; a NOT-configured provider gets the
// credentials form (not the dead-end `oauth_provider_not_configured`
// toast the user hit). It delegates to PlatformSignInButton, so we assert
// on what that renders per summary.

const getConfig = vi.fn();
vi.mock('@/lib/client', () => ({
  api: {
    oauth: {
      getConfig: () => getConfig(),
      startFlow: vi.fn(),
      setProviderCredentials: vi.fn(),
    },
  },
}));
vi.mock('@/hooks/useToast', () => ({
  toast: { error: vi.fn(), success: vi.fn(), info: vi.fn() },
}));
vi.mock('@/lib/logger', () => ({ logger: { error: vi.fn() } }));

import { ChatReauthBanner } from './ChatReauthBanner';
import { useProfileStore } from '@/stores/profileStore';
import { useChatStore } from '@/stores/chatStore';
import { createDefaultChatSettings } from '@/lib/profile-helpers';

function twitchSummary(configured: boolean): OAuthProviderSummary {
  return {
    provider: 'twitch',
    configured,
    needsSecret: false,
    overrideClientId: null,
    registrationUrl: 'https://dev.twitch.tv/console',
    setup: { steps: [], consoleFields: [] },
  };
}

const readOnlyTwitch: ChatPlatformStatus = {
  platform: 'twitch',
  status: 'connected',
  messageCount: 0,
  error: null,
  lastActivityMs: null,
  canSend: false,
};

beforeEach(() => {
  getConfig.mockReset();
  useChatStore.setState({ twitchReauthNeeded: false });
  useProfileStore.setState({
    current: {
      id: 'p',
      name: 'main',
      settings: { chat: { ...createDefaultChatSettings(), twitchChannel: 'mychan' } },
      outputGroups: [],
    },
  } as never);
});

describe('ChatReauthBanner', () => {
  it('renders nothing when no platform needs re-auth', () => {
    getConfig.mockResolvedValue([twitchSummary(true)]);
    const { container } = render(<ChatReauthBanner statuses={[{ ...readOnlyTwitch, canSend: true }]} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('shows the sign-in affordance for a configured read-only platform', async () => {
    getConfig.mockResolvedValue([twitchSummary(true)]);
    render(<ChatReauthBanner statuses={[readOnlyTwitch]} />);
    // Prompt shows immediately; the sign-in button appears once the summary loads.
    expect(
      screen.getByText('Sign in to Twitch again to send messages.')
    ).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /sign in/i })).toBeInTheDocument()
    );
  });

  it('guides to setup (no dead-end) when the provider is NOT configured', async () => {
    getConfig.mockResolvedValue([twitchSummary(false)]);
    render(<ChatReauthBanner statuses={[readOnlyTwitch]} />);
    // PlatformSignInButton renders the one-time-setup path + credentials form
    // instead of a sign-in that the backend would refuse.
    await waitFor(() =>
      expect(screen.getByText(/one-time setup/i)).toBeInTheDocument()
    );
  });
});
