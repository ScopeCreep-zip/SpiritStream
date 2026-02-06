import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { ChatMessage } from '@/types/chat';

const MAX_MESSAGES = 500;

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
          messages: [...state.messages, message].slice(-MAX_MESSAGES),
        })),

      addMessages: (messages) =>
        set((state) => ({
          messages: [...state.messages, ...messages].slice(-MAX_MESSAGES),
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
