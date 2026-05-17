import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { ChatMessage } from '@spiritstream/types';

const MAX_MESSAGES = 500;

const hasMessageId = (existing: readonly ChatMessage[], id: string): boolean =>
  existing.some((m) => m.id === id);

const dedupeAgainst = (existing: readonly ChatMessage[]) =>
  (incoming: ChatMessage): boolean => !hasMessageId(existing, incoming.id);

interface ChatStore {
  messages: ChatMessage[];
  overlayTransparent: boolean;
  overlayAlwaysOnTop: boolean;

  addMessage: (message: ChatMessage) => void;
  addMessages: (messages: ChatMessage[]) => void;
  clearMessages: () => void;
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
