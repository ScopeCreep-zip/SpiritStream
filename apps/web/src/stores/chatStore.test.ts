import { describe, it, expect, beforeEach } from 'vitest';
import type { ChatMessage, ChatAuthor } from '@spiritstream/types';
import { useChatStore } from './chatStore';
import { MessageFlag, hasFlag } from '@/lib/messageFlags';

// N5: chat message store. Its load-bearing logic is (1) the 500-message
// bounded buffer, (2) id-dedup so a reconnect replay doesn't double
// every line, and (3) the moderation flag mutations (CLEARMSG /
// CLEARCHAT) that dim deleted messages and timed-out authors. A
// regression in dedup floods the overlay; a regression in the flag
// mutation silently un-dims abuse the streamer asked to hide.

function makeMessage(overrides: Partial<ChatMessage> = {}): ChatMessage {
  return {
    id: 'twitch:1',
    platform: 'twitch',
    accountId: null,
    channelId: null,
    platforms: null,
    username: 'alice',
    message: 'hello',
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

function makeAuthor(login: string): ChatAuthor {
  return { userId: 'u-' + login, login, displayName: login, color: null, badgesRaw: [] };
}

beforeEach(() => {
  useChatStore.setState({ messages: [] });
});

describe('chatStore.addMessage', () => {
  it('appends a message', () => {
    useChatStore.getState().addMessage(makeMessage({ id: 'twitch:a' }));
    expect(useChatStore.getState().messages).toHaveLength(1);
  });

  it('ignores a duplicate id (reconnect replay must not double rows)', () => {
    const { addMessage } = useChatStore.getState();
    addMessage(makeMessage({ id: 'twitch:dup', message: 'first' }));
    addMessage(makeMessage({ id: 'twitch:dup', message: 'second' }));
    const { messages } = useChatStore.getState();
    expect(messages).toHaveLength(1);
    // The original is kept; the duplicate is dropped, not merged.
    expect(messages[0].message).toBe('first');
  });

  it('caps the buffer at 500, dropping oldest first', () => {
    const { addMessage } = useChatStore.getState();
    for (let i = 0; i < 600; i++) addMessage(makeMessage({ id: `twitch:${i}` }));
    const { messages } = useChatStore.getState();
    expect(messages).toHaveLength(500);
    expect(messages[0].id).toBe('twitch:100');
    expect(messages[messages.length - 1].id).toBe('twitch:599');
  });
});

describe('chatStore.addMessages', () => {
  it('dedupes a bulk insert against existing ids', () => {
    useChatStore.getState().addMessage(makeMessage({ id: 'twitch:1' }));
    useChatStore.getState().addMessages([
      makeMessage({ id: 'twitch:1' }), // dup of existing
      makeMessage({ id: 'twitch:2' }),
      makeMessage({ id: 'twitch:3' }),
    ]);
    const ids = useChatStore.getState().messages.map((m) => m.id);
    expect(ids).toEqual(['twitch:1', 'twitch:2', 'twitch:3']);
  });

  it('caps a bulk insert at the most recent 500', () => {
    const batch = Array.from({ length: 700 }, (_, i) => makeMessage({ id: `twitch:${i}` }));
    useChatStore.getState().addMessages(batch);
    const { messages } = useChatStore.getState();
    expect(messages).toHaveLength(500);
    expect(messages[0].id).toBe('twitch:200');
    expect(messages[messages.length - 1].id).toBe('twitch:699');
  });
});

describe('chatStore.markMessageDeleted', () => {
  it('sets the DISABLED flag on the matching message only', () => {
    useChatStore.getState().addMessages([
      makeMessage({ id: 'twitch:keep' }),
      makeMessage({ id: 'twitch:gone' }),
    ]);
    useChatStore.getState().markMessageDeleted('twitch:gone');
    const byId = Object.fromEntries(useChatStore.getState().messages.map((m) => [m.id, m]));
    expect(hasFlag(byId['twitch:gone'].flags, MessageFlag.DISABLED)).toBe(true);
    expect(hasFlag(byId['twitch:keep'].flags, MessageFlag.DISABLED)).toBe(false);
  });

  it('preserves existing flags when adding DISABLED', () => {
    useChatStore
      .getState()
      .addMessage(makeMessage({ id: 'twitch:hl', flags: MessageFlag.HIGHLIGHTED }));
    useChatStore.getState().markMessageDeleted('twitch:hl');
    const m = useChatStore.getState().messages[0];
    expect(hasFlag(m.flags, MessageFlag.HIGHLIGHTED)).toBe(true);
    expect(hasFlag(m.flags, MessageFlag.DISABLED)).toBe(true);
  });

  it('is a no-op when no message matches the id', () => {
    useChatStore.getState().addMessage(makeMessage({ id: 'twitch:1', flags: 0 }));
    useChatStore.getState().markMessageDeleted('twitch:absent');
    expect(useChatStore.getState().messages[0].flags).toBe(0);
  });
});

describe('chatStore.markUserTimedOut', () => {
  it('flags every past message from the author by structured login (case-insensitive)', () => {
    useChatStore.getState().addMessages([
      makeMessage({ id: '1', author: makeAuthor('troll') }),
      makeMessage({ id: '2', author: makeAuthor('innocent') }),
      makeMessage({ id: '3', author: makeAuthor('troll') }),
    ]);
    useChatStore.getState().markUserTimedOut('TROLL');
    const flagged = useChatStore
      .getState()
      .messages.filter((m) => hasFlag(m.flags, MessageFlag.TIMED_OUT_AUTHOR))
      .map((m) => m.id);
    expect(flagged).toEqual(['1', '3']);
  });

  it('falls back to the legacy username when author is absent', () => {
    useChatStore
      .getState()
      .addMessage(makeMessage({ id: '1', author: null, username: 'LegacyUser' }));
    useChatStore.getState().markUserTimedOut('legacyuser');
    expect(hasFlag(useChatStore.getState().messages[0].flags, MessageFlag.TIMED_OUT_AUTHOR)).toBe(
      true
    );
  });
});

describe('chatStore.clearMessages', () => {
  it('empties the buffer', () => {
    useChatStore.getState().addMessage(makeMessage());
    useChatStore.getState().clearMessages();
    expect(useChatStore.getState().messages).toHaveLength(0);
  });
});

describe('chatStore.twitchReauthNeeded', () => {
  it('defaults false and toggles via setter', () => {
    useChatStore.setState({ twitchReauthNeeded: false });
    expect(useChatStore.getState().twitchReauthNeeded).toBe(false);
    useChatStore.getState().setTwitchReauthNeeded(true);
    expect(useChatStore.getState().twitchReauthNeeded).toBe(true);
    useChatStore.getState().setTwitchReauthNeeded(false);
    expect(useChatStore.getState().twitchReauthNeeded).toBe(false);
  });
});
