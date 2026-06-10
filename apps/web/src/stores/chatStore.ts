import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { ChatMessage } from '@spiritstream/types';
import { MessageFlag } from '@/lib/messageFlags';

const MAX_MESSAGES = 500;

const hasMessageId = (existing: readonly ChatMessage[], id: string): boolean =>
  existing.some((m) => m.id === id);

const dedupeAgainst =
  (existing: readonly ChatMessage[]) =>
  (incoming: ChatMessage): boolean =>
    !hasMessageId(existing, incoming.id);

interface ChatStore {
  messages: ChatMessage[];
  overlayTransparent: boolean;
  overlayAlwaysOnTop: boolean;

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
  setOverlayTransparent: (transparent: boolean) => void;
  setOverlayAlwaysOnTop: (alwaysOnTop: boolean) => void;
}

export const useChatStore = create<ChatStore>()(
  persist(
    (set) => ({
      messages: [],
      overlayTransparent: false,
      overlayAlwaysOnTop: true,

      addMessage: (message) =>
        set((state) => ({
          messages: hasMessageId(state.messages, message.id)
            ? state.messages
            : [...state.messages, message].slice(-MAX_MESSAGES),
        })),

      addMessages: (messages) =>
        set((state) => ({
          messages: [...state.messages, ...messages.filter(dedupeAgainst(state.messages))].slice(
            -MAX_MESSAGES
          ),
        })),

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

      setOverlayTransparent: (transparent) => set({ overlayTransparent: transparent }),

      setOverlayAlwaysOnTop: (alwaysOnTop) => set({ overlayAlwaysOnTop: alwaysOnTop }),
    }),
    {
      name: 'spiritstream-chat',
      partialize: (state) => ({
        overlayTransparent: state.overlayTransparent,
        overlayAlwaysOnTop: state.overlayAlwaysOnTop,
      }),
    }
  )
);
