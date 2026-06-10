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
    // Unmount can race the async registrations; the cancelled flag makes
    // a late-resolving subscribe release itself instead of leaking.
    let cancelled = false;
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
        if (cancelled) {
          unsubscribe();
          return;
        }
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
        if (cancelled) {
          unsubscribe();
          return;
        }
        unlistenOverlay = unsubscribe;
      })
      .catch((error) => {
        logger.error('Failed to listen for chat overlay settings:', error);
      });

    return () => {
      cancelled = true;
      if (unlistenMessages) {
        unlistenMessages();
      }
      if (unlistenOverlay) {
        unlistenOverlay();
      }
    };
  }, [addMessage, markMessageDeleted, markUserTimedOut, setOverlayTransparent]);
}
