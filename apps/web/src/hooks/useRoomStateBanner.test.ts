import { describe, it, expect } from 'vitest';
import { renderHook } from '@testing-library/react';
import type { ChatMessage, RoomStatePayload } from '@spiritstream/types';
import { useRoomStateBanner } from './useRoomStateBanner';

// The banner folds Twitch ROOMSTATE deltas into the current channel-mode
// state. The load-bearing rules: a `null` field means "unchanged" (not
// "off"), `followersOnlyDisabled` wins over `followersOnlyMinutes`, and
// slowMode 0 means off. A fold bug shows a stale/wrong mode banner.

function roomState(partial: Partial<RoomStatePayload>): ChatMessage {
  const payload: RoomStatePayload = {
    emoteOnly: null,
    subscribersOnly: null,
    r9k: null,
    slowModeSecs: null,
    followersOnlyMinutes: null,
    followersOnlyDisabled: null,
    ...partial,
  };
  return {
    id: `rs:${Math.round(payload.slowModeSecs ?? 0)}:${JSON.stringify(partial)}`,
    platform: 'twitch',
    accountId: null,
    channelId: null,
    platforms: null,
    username: 'Twitch',
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
    event: { kind: 'roomStateChanged', ...payload },
    direction: 'inbound',
    sourceId: null,
    color: null,
    badges: null,
  };
}

function plainMessage(): ChatMessage {
  return { ...roomState({}), id: 'plain', message: 'hi', event: null };
}

describe('useRoomStateBanner', () => {
  it('returns all-off for no roomstate messages', () => {
    const { result } = renderHook(() => useRoomStateBanner([plainMessage()]));
    expect(result.current).toEqual({
      emoteOnly: false,
      subscribersOnly: false,
      r9k: false,
      slowModeSecs: 0,
      followersOnlyMinutes: null,
    });
  });

  it('folds deltas, leaving untouched (null) fields unchanged', () => {
    const messages = [
      roomState({ subscribersOnly: true, slowModeSecs: 30 }),
      roomState({ emoteOnly: true }), // does not clear subscribersOnly/slowMode
    ];
    const { result } = renderHook(() => useRoomStateBanner(messages));
    expect(result.current.subscribersOnly).toBe(true);
    expect(result.current.slowModeSecs).toBe(30);
    expect(result.current.emoteOnly).toBe(true);
  });

  it('treats slowModeSecs 0 as off', () => {
    const messages = [roomState({ slowModeSecs: 30 }), roomState({ slowModeSecs: 0 })];
    const { result } = renderHook(() => useRoomStateBanner(messages));
    expect(result.current.slowModeSecs).toBe(0);
  });

  it('followersOnlyDisabled overrides followersOnlyMinutes', () => {
    const on = renderHook(() => useRoomStateBanner([roomState({ followersOnlyMinutes: 10 })]));
    expect(on.result.current.followersOnlyMinutes).toBe(10);

    const off = renderHook(() =>
      useRoomStateBanner([
        roomState({ followersOnlyMinutes: 10 }),
        roomState({ followersOnlyMinutes: 10, followersOnlyDisabled: true }),
      ])
    );
    expect(off.result.current.followersOnlyMinutes).toBeNull();
  });

  it('keeps followersOnlyMinutes 0 (any-follower) distinct from off', () => {
    const { result } = renderHook(() =>
      useRoomStateBanner([roomState({ followersOnlyMinutes: 0 })])
    );
    expect(result.current.followersOnlyMinutes).toBe(0);
  });
});
