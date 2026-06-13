import { useMemo } from 'react';
import type { ChatMessage } from '@spiritstream/types';

/**
 * Aggregate channel-mode state, folded from the stream of Twitch ROOMSTATE
 * deltas. Twitch sends a full baseline on join and incremental deltas after;
 * a `null` field on a delta means "unchanged", NOT "off".
 */
export interface RoomMode {
  emoteOnly: boolean;
  subscribersOnly: boolean;
  r9k: boolean;
  /** Seconds; 0 = off. */
  slowModeSecs: number;
  /** Minutes; `null` = off, 0 = any follower may chat, N = follower ≥ N min. */
  followersOnlyMinutes: number | null;
}

const INITIAL: RoomMode = {
  emoteOnly: false,
  subscribersOnly: false,
  r9k: false,
  slowModeSecs: 0,
  followersOnlyMinutes: null,
};

/**
 * Derive the current channel-mode banner state by folding every
 * `roomStateChanged` event in `messages`. Pure + memoized — the deltas live
 * in `chatStore.messages` (which survive reconnect replay), so the banner
 * reconstructs correctly after a refresh without extra store state. ROOMSTATE
 * rows are filtered out of the visible list (ChatList); this is where they go.
 */
export function useRoomStateBanner(messages: ChatMessage[]): RoomMode {
  return useMemo(() => {
    let mode: RoomMode = { ...INITIAL };
    for (const msg of messages) {
      const e = msg.event;
      if (e?.kind !== 'roomStateChanged') continue;
      if (e.emoteOnly !== null) mode = { ...mode, emoteOnly: e.emoteOnly };
      if (e.subscribersOnly !== null) mode = { ...mode, subscribersOnly: e.subscribersOnly };
      if (e.r9k !== null) mode = { ...mode, r9k: e.r9k };
      if (e.slowModeSecs !== null) mode = { ...mode, slowModeSecs: e.slowModeSecs };
      // `followersOnlyDisabled` disambiguates "off" from "any-follower / N-min".
      if (e.followersOnlyDisabled === true) {
        mode = { ...mode, followersOnlyMinutes: null };
      } else if (e.followersOnlyMinutes !== null) {
        mode = { ...mode, followersOnlyMinutes: e.followersOnlyMinutes };
      }
    }
    return mode;
  }, [messages]);
}
