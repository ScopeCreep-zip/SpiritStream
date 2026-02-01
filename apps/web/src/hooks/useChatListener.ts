import { useEffect } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useChatStore } from '@/stores/chatStore';
import type { ChatMessage } from '@/types/chat';
import { CHAT_MESSAGE_EVENT, CHAT_OVERLAY_SETTINGS_EVENT } from '@/lib/chatEvents';

export function useChatListener() {
  const addMessage = useChatStore((state) => state.addMessage);
  const setOverlayTransparent = useChatStore((state) => state.setOverlayTransparent);

  useEffect(() => {
    let unlistenMessages: UnlistenFn | null = null;
    let unlistenOverlay: UnlistenFn | null = null;

    listen<ChatMessage>(CHAT_MESSAGE_EVENT, (event) => {
      addMessage(event.payload);
    })
      .then((unsubscribe) => {
        unlistenMessages = unsubscribe;
      })
      .catch((error) => {
        console.error('Failed to listen for chat messages:', error);
      });

    listen<{ transparent: boolean }>(CHAT_OVERLAY_SETTINGS_EVENT, (event) => {
      setOverlayTransparent(event.payload.transparent);
    })
      .then((unsubscribe) => {
        unlistenOverlay = unsubscribe;
      })
      .catch((error) => {
        console.error('Failed to listen for chat overlay settings:', error);
      });

    return () => {
      if (unlistenMessages) {
        unlistenMessages();
      }
      if (unlistenOverlay) {
        unlistenOverlay();
      }
    };
  }, [addMessage, setOverlayTransparent]);
}
