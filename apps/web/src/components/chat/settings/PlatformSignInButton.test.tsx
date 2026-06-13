import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import '@/lib/i18n';
import type { OAuthProviderSummary } from '@spiritstream/api-client';

// When a token is dead (read-only) the stored username persists, so the
// button would normally show only "Sign out" — stranding the user. The
// `reauthAvailable` flag must surface "Sign in again" so they can recover
// without signing out first.

vi.mock('@/lib/client', () => ({
  api: { oauth: { startFlow: vi.fn(), forget: vi.fn() } },
}));
vi.mock('@/hooks/useToast', () => ({
  toast: { error: vi.fn(), success: vi.fn(), info: vi.fn() },
}));
vi.mock('@/lib/logger', () => ({ logger: { error: vi.fn() } }));

import { PlatformSignInButton } from './PlatformSignInButton';

const configured: OAuthProviderSummary = {
  provider: 'twitch',
  configured: true,
  needsSecret: false,
  overrideClientId: null,
  registrationUrl: 'https://dev.twitch.tv/console',
  setup: { steps: [], consoleFields: [] },
};

describe('PlatformSignInButton re-auth affordance', () => {
  it('shows "Sign in again" alongside "Sign out" when signed-in but read-only', () => {
    render(
      <PlatformSignInButton
        provider="twitch"
        signedInAs="alice"
        signInLabel="Login with Twitch"
        summary={configured}
        onCredentialsSaved={vi.fn()}
        reauthAvailable
      />
    );
    expect(screen.getByRole('button', { name: /sign in again/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /sign out/i })).toBeInTheDocument();
  });

  it('shows only "Sign out" when signed in and healthy', () => {
    render(
      <PlatformSignInButton
        provider="twitch"
        signedInAs="alice"
        signInLabel="Login with Twitch"
        summary={configured}
        onCredentialsSaved={vi.fn()}
      />
    );
    expect(screen.queryByRole('button', { name: /sign in again/i })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /sign out/i })).toBeInTheDocument();
  });
});
