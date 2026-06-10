import { useEffect } from 'react';
import { events } from '@spiritstream/api-client';
import { logger } from '@/lib/logger';
import { useChatStore } from '@/stores/chatStore';
import type { ChatMessage } from '@spiritstream/types';
import { CHAT_MESSAGE_EVENT, CHAT_OVERLAY_SETTINGS_EVENT } from '@/lib/chatEvents';

type UnlistenFn = () => void;

export function useChatListener() {
  const addMessage = useChatStore((state) => state.addMessage);
  const markMessageDeleted = useChatStore((state) => state.markMessageDeleted);
  const markUserTimedOut = useChatStore((state) => state.markUserTimedOut);
  const setOverlayTransparent = useChatStore((state) => state.setOverlayTransparent);

  useEffect(() => {
    let unlistenMessages: UnlistenFn | null = null;
    let unlistenOverlay: UnlistenFn | null = null;

    events
      .on<ChatMessage>(CHAT_MESSAGE_EVENT, (payload) => {
        // Plan B4 contract: MessageDeleted and UserBanned events mutate
        // PAST messages rather than appending a new row. Every other
        // ChatEvent variant (SubGifted, Raid, MemberMilestone,
        // RoomStateChanged) flows through as a normal system row.
        const evt = payload.event;
        if (evt) {
          if (evt.kind === 'messageDeleted') {
            markMessageDeleted(evt.id);
            return;
          }
          if (evt.kind === 'userBanned') {
            markUserTimedOut(evt.userLogin);
            return;
          }
        }
        addMessage(payload);
      })
      .then((unsubscribe) => {
        unlistenMessages = unsubscribe;
      })
      .catch((error) => {
        logger.error('Failed to listen for chat messages:', error);
      });

    events
      .on<{ transparent: boolean }>(CHAT_OVERLAY_SETTINGS_EVENT, (payload) => {
        setOverlayTransparent(payload.transparent);
      })
      .then((unsubscribe) => {
        unlistenOverlay = unsubscribe;
      })
      .catch((error) => {
        logger.error('Failed to listen for chat overlay settings:', error);
      });

    return () => {
      if (unlistenMessages) {
        unlistenMessages();
      }
      if (unlistenOverlay) {
        unlistenOverlay();
      }
    };
  }, [addMessage, markMessageDeleted, markUserTimedOut, setOverlayTransparent]);
}
