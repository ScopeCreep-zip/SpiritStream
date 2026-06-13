import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import '@/lib/i18n';
import type { ChatPlatformStatus } from '@spiritstream/types';

// The re-auth banner is a thin pointer: when a platform is connected but
// can't send, it prompts and routes the user to the Integrations panel
// (which owns the full OAuth setup + sign-in). It must NOT inline the
// setup form, and must call onOpenIntegrations on click.

import { ChatReauthBanner } from './ChatReauthBanner';
import { useProfileStore } from '@/stores/profileStore';
import { useChatStore } from '@/stores/chatStore';
import { createDefaultChatSettings } from '@/lib/profile-helpers';

const readOnlyTwitch: ChatPlatformStatus = {
  platform: 'twitch',
  status: 'connected',
  messageCount: 0,
  error: null,
  lastActivityMs: null,
  canSend: false,
};

beforeEach(() => {
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
    const { container } = render(
      <ChatReauthBanner statuses={[{ ...readOnlyTwitch, canSend: true }]} onOpenIntegrations={vi.fn()} />
    );
    expect(container).toBeEmptyDOMElement();
  });

  it('renders nothing when no opener is provided (cannot route)', () => {
    const { container } = render(<ChatReauthBanner statuses={[readOnlyTwitch]} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('prompts and routes to Integrations for a read-only platform', () => {
    const onOpenIntegrations = vi.fn();
    render(<ChatReauthBanner statuses={[readOnlyTwitch]} onOpenIntegrations={onOpenIntegrations} />);
    expect(screen.getByText(/sign in to twitch again to send/i)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /open settings/i }));
    expect(onOpenIntegrations).toHaveBeenCalledOnce();
  });
});
