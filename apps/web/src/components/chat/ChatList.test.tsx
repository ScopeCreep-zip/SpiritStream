import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import '@/lib/i18n';
import type { ChatEvent, ChatMessage } from '@spiritstream/types';
import { ChatList } from './ChatList';

// ChatList branches on message.event: ROOMSTATE feeds the banner (no row),
// other events render a notice, plain messages render their text/fragments.
// The user's "blank twitch row" bug was a ROOMSTATE event rendered as a
// bare username row — these pin the branch that fixed it.

function message(overrides: Partial<ChatMessage>): ChatMessage {
  return {
    id: 'm',
    platform: 'twitch',
    accountId: null,
    channelId: null,
    platforms: null,
    username: 'alice',
    message: '',
    timestamp: 0,
    serverReceivedAtMs: null,
    author: null,
    fragments: [],
    flags: 0,
    rawText: null,
    bitsTotal: null,
    highlightColor: null,
    elevatedTier: null,
    reply: null,
    event: null,
    direction: 'inbound',
    sourceId: null,
    color: null,
    badges: null,
    ...overrides,
  };
}

const roomState: ChatEvent = {
  kind: 'roomStateChanged',
  emoteOnly: true,
  subscribersOnly: null,
  r9k: null,
  slowModeSecs: null,
  followersOnlyMinutes: null,
  followersOnlyDisabled: null,
};

describe('ChatList event branching', () => {
  it('renders a plain message as its text, not via a notice', () => {
    render(<ChatList messages={[message({ id: 'a', message: 'hello world' })]} />);
    expect(screen.getByText('hello world')).toBeInTheDocument();
  });

  it('renders a raid event as a notice row', () => {
    render(
      <ChatList
        messages={[
          message({
            id: 'r',
            event: { kind: 'raid', raiderLogin: 'b', raiderDisplayName: 'Bob', viewerCount: 9 },
          }),
        ]}
      />
    );
    expect(screen.getByText('Bob raided with 9 viewers')).toBeInTheDocument();
  });

  it('does not render a row for a roomStateChanged event', () => {
    const { container } = render(
      <ChatList messages={[message({ id: 'rs', username: 'Twitch', event: roomState })]} />
    );
    // No author/username row, no event wrapper — it feeds the banner instead.
    expect(screen.queryByText('Twitch')).not.toBeInTheDocument();
    expect(container.querySelector('[data-event]')).toBeNull();
  });
});
