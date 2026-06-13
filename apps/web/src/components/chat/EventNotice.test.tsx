import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import '@/lib/i18n';
import type { ChatEvent, ChatMessage } from '@spiritstream/types';
import { EventNotice } from './EventNotice';

// EventNotice turns a ChatEvent into a localized notice row. A regression
// here is what the user reported as a blank "twitch" row — an event that
// renders nothing meaningful. These pin that each kind produces readable
// text, that author-bearing events use the message author, and that the
// banner/mutation events render no row.

function message(event: ChatEvent | null, overrides: Partial<ChatMessage> = {}): ChatMessage {
  return {
    id: 'twitch:1',
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
    event,
    direction: 'inbound',
    sourceId: null,
    color: null,
    badges: null,
    ...overrides,
  };
}

describe('EventNotice', () => {
  it('renders a raid notice with the raider and viewer count', () => {
    render(
      <EventNotice
        message={message({
          kind: 'raid',
          raiderLogin: 'bob',
          raiderDisplayName: 'Bob',
          viewerCount: 42,
        })}
      />
    );
    expect(screen.getByText('Bob raided with 42 viewers')).toBeInTheDocument();
  });

  it('uses the message author for author-bearing events (cheer)', () => {
    render(
      <EventNotice
        message={message(
          { kind: 'cheer', bits: 100, userTotalBits: null },
          { author: { userId: '1', login: 'alice', displayName: 'Alice', color: null, badgesRaw: [] } }
        )}
      />
    );
    expect(screen.getByText('Alice cheered 100 bits')).toBeInTheDocument();
  });

  it('renders a gifted-sub notice', () => {
    render(
      <EventNotice
        message={message({
          kind: 'subGifted',
          gifterLogin: 'carol',
          gifterDisplayName: 'Carol',
          count: 5,
          recipientLogins: [],
          tier: '1000',
        })}
      />
    );
    expect(screen.getByText('Carol gifted 5 sub(s)')).toBeInTheDocument();
  });

  it('renders no row for roomStateChanged (handled by the banner)', () => {
    const { container } = render(
      <EventNotice
        message={message({
          kind: 'roomStateChanged',
          emoteOnly: true,
          subscribersOnly: null,
          r9k: null,
          slowModeSecs: null,
          followersOnlyMinutes: null,
          followersOnlyDisabled: null,
        })}
      />
    );
    expect(container).toBeEmptyDOMElement();
  });

  it('renders no row when there is no event', () => {
    const { container } = render(<EventNotice message={message(null)} />);
    expect(container).toBeEmptyDOMElement();
  });
});
