import { useEffect } from 'react';
import { events } from '@/lib/backend';
import { useChatStore } from '@/stores/chatStore';
import type { ChatMessage } from '@/types/chat';
import { CHAT_MESSAGE_EVENT, CHAT_OVERLAY_SETTINGS_EVENT } from '@/lib/chatEvents';

type UnlistenFn = () => void;

export function useChatListener() {
  const addMessage = useChatStore((state) => state.addMessage);
  const setOverlayTransparent = useChatStore((state) => state.setOverlayTransparent);

  useEffect(() => {
    let unlistenMessages: UnlistenFn | null = null;
    let unlistenOverlay: UnlistenFn | null = null;

    events.on<ChatMessage>(CHAT_MESSAGE_EVENT, (payload) => {
      addMessage(payload);
    })
      .then((unsubscribe) => {
        unlistenMessages = unsubscribe;
      })
      .catch((error) => {
        console.error('Failed to listen for chat messages:', error);
      });

    events.on<{ transparent: boolean }>(CHAT_OVERLAY_SETTINGS_EVENT, (payload) => {
      setOverlayTransparent(payload.transparent);
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
