import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import '@/lib/i18n';
import type { ChatPlatformStatus } from '@spiritstream/types';

// A connected platform is NOT automatically a send target: the Twitch
// anonymous read-only fallback (and expired tokens) connect with
// `canSend === false`. The composer must gate on `canSend`, not just
// `status === 'connected'`, or it offers a send the server rejects — the
// exact bug the user hit. These pin the gate + the re-auth hint.

vi.mock('@/lib/client', () => ({ api: { chat: { sendMessage: vi.fn() } } }));
vi.mock('@/hooks/useToast', () => ({
  toast: { error: vi.fn(), success: vi.fn(), info: vi.fn() },
}));
vi.mock('@/lib/logger', () => ({ logger: { error: vi.fn() } }));

import { ChatComposer } from './ChatComposer';
import { useProfileStore } from '@/stores/profileStore';
import { useChatStore } from '@/stores/chatStore';
import { createDefaultChatSettings } from '@/lib/profile-helpers';

function twitchStatus(canSend: boolean): ChatPlatformStatus {
  return {
    platform: 'twitch',
    status: 'connected',
    messageCount: 0,
    error: null,
    lastActivityMs: null,
    canSend,
  };
}

beforeEach(() => {
  useChatStore.setState({ twitchReauthNeeded: false });
  useProfileStore.setState({
    current: {
      id: 'p',
      name: 'main',
      settings: {
        chat: { ...createDefaultChatSettings(), twitchChannel: 'mychan', twitchSendEnabled: true },
      },
      outputGroups: [],
    },
  } as never);
});

describe('ChatComposer send gating', () => {
  it('blocks send for a connected read-only platform and shows the re-auth hint', () => {
    render(<ChatComposer statuses={[twitchStatus(false)]} />);
    expect(screen.getByRole('textbox')).toBeDisabled();
    expect(
      screen.getByText('Your chat sign-in is read-only — sign in again to send.')
    ).toBeInTheDocument();
  });

  it('allows send when the platform is authorized (canSend)', () => {
    render(<ChatComposer statuses={[twitchStatus(true)]} />);
    expect(screen.getByRole('textbox')).not.toBeDisabled();
  });

  it('blocks send when twitchReauthNeeded even if canSend is still stale-true', () => {
    useChatStore.setState({ twitchReauthNeeded: true });
    render(<ChatComposer statuses={[twitchStatus(true)]} />);
    expect(screen.getByRole('textbox')).toBeDisabled();
  });
});
