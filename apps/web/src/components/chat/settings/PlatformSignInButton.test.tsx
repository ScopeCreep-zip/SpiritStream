import { beforeEach, describe, it, expect, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import '@/lib/i18n';
import type { OAuthProviderSummary } from '@spiritstream/api-client';

// When a token is dead (read-only) the stored username persists, so the
// button would normally show only "Sign out" — stranding the user. The
// `reauthAvailable` flag must surface "Sign in again" so they can recover
// without signing out first.

const { startFlowMock, forgetMock } = vi.hoisted(() => ({
  startFlowMock: vi.fn(),
  forgetMock: vi.fn(),
}));

vi.mock('@/lib/client', () => ({
  api: { oauth: { startFlow: startFlowMock, forget: forgetMock } },
}));
vi.mock('@/hooks/useToast', () => ({
  toast: { error: vi.fn(), success: vi.fn(), info: vi.fn() },
}));
vi.mock('@/lib/logger', () => ({ logger: { error: vi.fn() } }));
vi.mock('@/stores/profileStore', () => ({
  useProfileStore: {
    getState: vi.fn(() => ({ current: null })),
    setState: vi.fn(),
  },
}));
vi.mock('@/lib/profile-helpers', () => ({
  createDefaultOAuthAccount: vi.fn(() => ({
    accessToken: '',
    refreshToken: '',
    expiresAt: 0,
    userId: '',
    username: '',
    displayName: '',
  })),
}));

import { PlatformSignInButton } from './PlatformSignInButton';

const configured: OAuthProviderSummary = {
  provider: 'twitch',
  configured: true,
  needsSecret: false,
  overrideClientId: null,
  registrationUrl: 'https://dev.twitch.tv/console',
  setup: { steps: [], consoleFields: [] },
};

describe('PlatformSignInButton account state', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('flushes pending parent edits before starting sign-in', async () => {
    const beforeSignIn = vi.fn().mockResolvedValue(undefined);
    startFlowMock.mockResolvedValue({
      flow: 'redirect',
      browserOpened: true,
    });

    render(
      <PlatformSignInButton
        provider="twitch"
        signedInAs=""
        signInLabel="Login with Twitch"
        summary={configured}
        onCredentialsSaved={vi.fn()}
        beforeSignIn={beforeSignIn}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /login with twitch/i }));

    await waitFor(() => expect(beforeSignIn).toHaveBeenCalledTimes(1));
    expect(startFlowMock).toHaveBeenCalledWith('twitch');
  });

  it('makes "Sign back in" the primary action when connected read-only (dead token)', () => {
    render(
      <PlatformSignInButton
        provider="twitch"
        signedInAs="alice"
        signInLabel="Login with Twitch"
        summary={configured}
        onCredentialsSaved={vi.fn()}
        connectionStatus="connected"
        canSend={false}
      />
    );
    expect(screen.getByRole('button', { name: /sign back in/i })).toBeInTheDocument();
    // The read-only situation is explained in-place, not via a stray flag.
    expect(screen.getByText(/sign-in expired/i)).toBeInTheDocument();
    // Sign out is demoted to a secondary affordance, not the primary.
    expect(screen.getByRole('button', { name: /sign out/i })).toBeInTheDocument();
  });

  it('shows only "Sign out" when signed in and send-capable', () => {
    render(
      <PlatformSignInButton
        provider="twitch"
        signedInAs="alice"
        signInLabel="Login with Twitch"
        summary={configured}
        onCredentialsSaved={vi.fn()}
        connectionStatus="connected"
        canSend={true}
      />
    );
    expect(screen.queryByRole('button', { name: /sign back in/i })).not.toBeInTheDocument();
    expect(screen.getByText(/signed in as alice/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /sign out/i })).toBeInTheDocument();
  });

  it('routes a read-only account to setup (not a failing "Sign back in") when the provider has no credentials', () => {
    const notConfigured = { ...configured, configured: false };
    render(
      <PlatformSignInButton
        provider="twitch"
        signedInAs="alice"
        signInLabel="Login with Twitch"
        summary={notConfigured}
        onCredentialsSaved={vi.fn()}
        connectionStatus="connected"
        canSend={false}
      />
    );
    // No "Sign back in" — it would dead-end on oauth_provider_not_configured.
    expect(screen.queryByRole('button', { name: /sign back in/i })).not.toBeInTheDocument();
    expect(screen.getByText(/needs a one-time setup for this platform/i)).toBeInTheDocument();
  });

  it('shows "Sign in" when no account is stored', () => {
    render(
      <PlatformSignInButton
        provider="twitch"
        signedInAs=""
        signInLabel="Login with Twitch"
        summary={configured}
        onCredentialsSaved={vi.fn()}
      />
    );
    expect(screen.getByRole('button', { name: /login with twitch/i })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /sign out/i })).not.toBeInTheDocument();
  });
});
