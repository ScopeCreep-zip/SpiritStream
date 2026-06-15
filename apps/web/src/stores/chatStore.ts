import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { ChatMessage } from '@spiritstream/types';
import { MessageFlag } from '@/lib/messageFlags';

const MAX_MESSAGES = 500;

const hasMessageId = (existing: readonly ChatMessage[], id: string): boolean =>
  existing.some((m) => m.id === id);

interface ChatStore {
  messages: ChatMessage[];
  /** Overlay opacity (0 = fully see-through, 1 = opaque). Drives the
   *  `--overlay-opacity` CSS var; tuned by the transparency slider. Applies to
   *  the whole pop-out window background. */
  overlayOpacity: number;
  overlayAlwaysOnTop: boolean;
  /**
   * Set when the backend reports the follower-only default could not be
   * applied because the Twitch token lacks the moderator scope (event
   * `follower_only_unsupported`, reason `follower_only_missing_scope` /
   * `no_oauth_token`). Drives the "sign in with Twitch again" hint in
   * the chat settings panel. Not persisted — re-derived per session.
   */
  followerOnlyReauthNeeded: boolean;
  /**
   * Set when the backend reports a Twitch OAuth token expired (event
   * `oauth_token_expired`, provider `twitch`) during activation/refresh.
   * The IRC socket may stay open with a now-stale `canSend`, so this is
   * the immediate signal that sending needs a re-auth; the chat status's
   * `canSend` is the eventually-consistent one (flips on next reconnect).
   * Drives the inline "sign in to send" banner. Not persisted — re-derived
   * per session and cleared on `oauth_complete` for twitch.
   */
  twitchReauthNeeded: boolean;

  addMessage: (message: ChatMessage) => void;
  addMessages: (messages: ChatMessage[]) => void;
  clearMessages: () => void;
  /**
   * Mutates the existing message with `id` so its `flags` includes
   * `DISABLED`. Implements the plan's CLEARMSG contract — "set
   * `flags = DISABLED` on existing message by id (mutate, don't
   * replace)". The id is the platform-prefixed id the backend emits
   * (e.g. `"twitch:ABC123"`). No-op if no message matches.
   */
  markMessageDeleted: (id: string) => void;
  /**
   * Marks every past message authored by `login` with the
   * `TIMED_OUT_AUTHOR` flag. Implements the plan's CLEARCHAT
   * contract — "renderer dims past messages from that user". Match
   * uses `author.login` if present, otherwise the lowercased legacy
   * `username` (case-insensitive).
   */
  markUserTimedOut: (login: string) => void;
  setOverlayOpacity: (opacity: number) => void;
  setOverlayAlwaysOnTop: (alwaysOnTop: boolean) => void;
  setFollowerOnlyReauthNeeded: (needed: boolean) => void;
  setTwitchReauthNeeded: (needed: boolean) => void;
}

export const useChatStore = create<ChatStore>()(
  persist(
    (set) => ({
      messages: [],
      overlayOpacity: 1,
      overlayAlwaysOnTop: true,
      followerOnlyReauthNeeded: false,
      twitchReauthNeeded: false,

      addMessage: (message) =>
        set((state) => ({
          messages: hasMessageId(state.messages, message.id)
            ? state.messages
            : [...state.messages, message].slice(-MAX_MESSAGES),
        })),

      addMessages: (messages) =>
        set((state) => {
          // Dedup the incoming batch against existing AND against itself.
          // A server-side history replay can carry the same id more than
          // once (e.g. the persisted log retained duplicates from an
          // earlier connector bug); filtering only against `state.messages`
          // let same-batch repeats through, producing duplicate React keys
          // that break the feed's reconciliation + auto-scroll.
          const seen = new Set(state.messages.map((m) => m.id));
          const fresh: ChatMessage[] = [];
          for (const message of messages) {
            if (seen.has(message.id)) continue;
            seen.add(message.id);
            fresh.push(message);
          }
          return { messages: [...state.messages, ...fresh].slice(-MAX_MESSAGES) };
        }),

      clearMessages: () => set({ messages: [] }),

      markMessageDeleted: (id) =>
        set((state) => ({
          messages: state.messages.map((m) =>
            m.id === id ? { ...m, flags: (m.flags ?? 0) | MessageFlag.DISABLED } : m
          ),
        })),

      markUserTimedOut: (login) =>
        set((state) => {
          const targetLogin = login.toLowerCase();
          return {
            messages: state.messages.map((m) => {
              const authorLogin = (m.author?.login ?? m.username).toLowerCase();
              return authorLogin === targetLogin
                ? { ...m, flags: (m.flags ?? 0) | MessageFlag.TIMED_OUT_AUTHOR }
                : m;
            }),
          };
        }),

      setOverlayOpacity: (overlayOpacity) => set({ overlayOpacity }),

      setOverlayAlwaysOnTop: (alwaysOnTop) => set({ overlayAlwaysOnTop: alwaysOnTop }),

      setFollowerOnlyReauthNeeded: (needed) => set({ followerOnlyReauthNeeded: needed }),

      setTwitchReauthNeeded: (needed) => set({ twitchReauthNeeded: needed }),
    }),
    {
      name: 'spiritstream-chat',
      // v0: boolean `overlayTransparent`. v1: `overlayVariant` enum +
      // `overlayGlassOpacity`. v2: dropped the `transparent` variant (now
      // `normal` at opacity 0) + renamed opacity to `overlayOpacity`. v3:
      // dropped the overlay-variant concept entirely (no glass) — the pop-out
      // is just a transparency slider over a flat background.
      version: 3,
      migrate: (persisted, version) => {
        const p = (persisted ?? {}) as {
          overlayTransparent?: boolean;
          overlayVariant?: string;
          overlayGlassOpacity?: number;
          overlayOpacity?: number;
          overlayAlwaysOnTop?: boolean;
        };
        const overlayAlwaysOnTop = p.overlayAlwaysOnTop ?? true;
        if (version < 1) {
          return { overlayOpacity: p.overlayTransparent ? 0 : 1, overlayAlwaysOnTop };
        }
        if (version < 2) {
          // v1 had per-variant opacity in `overlayGlassOpacity`; collapse to a
          // single opacity (0 for the old `transparent` preset, else opaque).
          const overlayOpacity =
            p.overlayVariant === 'transparent' ? 0 : p.overlayGlassOpacity ?? 1;
          return { overlayOpacity, overlayAlwaysOnTop };
        }
        return { overlayOpacity: p.overlayOpacity ?? 1, overlayAlwaysOnTop };
      },
      // `messages` is DELIBERATELY excluded from localStorage. Inbound
      // chat is sensitive (harassment/doxxing content + PII) and the
      // webview renders chat from strangers — an XSS-exposed surface.
      // OWASP (HTML5 Cheat Sheet / ASVS V14) is explicit: sensitive data
      // belongs server-side, never in browser storage (one XSS drains
      // localStorage). History is repopulated from the backend's recent-
      // message endpoint on (re)connect instead; only the non-sensitive
      // overlay UI prefs persist here.
      partialize: (state) => ({
        overlayOpacity: state.overlayOpacity,
        overlayAlwaysOnTop: state.overlayAlwaysOnTop,
      }),
    }
  )
);
