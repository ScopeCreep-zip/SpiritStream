import { useEffect } from 'react';
import { events } from '@spiritstream/api-client';
import { api } from '@/lib/client';
import { logger } from '@/lib/logger';
import { useChatStore } from '@/stores/chatStore';
import type { ChatMessage } from '@spiritstream/types';
import { CHAT_MESSAGE_EVENT, CHAT_OVERLAY_SETTINGS_EVENT } from '@/lib/chatEvents';

type UnlistenFn = () => void;

export function useChatListener() {
  const addMessage = useChatStore((state) => state.addMessage);
  const addMessages = useChatStore((state) => state.addMessages);
  const markMessageDeleted = useChatStore((state) => state.markMessageDeleted);
  const markUserTimedOut = useChatStore((state) => state.markUserTimedOut);
  const setOverlayTransparent = useChatStore((state) => state.setOverlayTransparent);

  useEffect(() => {
    // Unmount can race the async registrations; the cancelled flag makes
    // a late-resolving subscribe release itself instead of leaking.
    let cancelled = false;
    let unlistenMessages: UnlistenFn | null = null;
    let unlistenOverlay: UnlistenFn | null = null;

    // Repopulate chat history from the BACKEND on first load and on every
    // (re)connect — the message store is in-memory only (OWASP: sensitive
    // chat never goes to browser storage), so a refresh / WS reconnect
    // needs a server-side replay. `addMessages` dedups by id, so overlap
    // with live events collapses harmlessly.
    const seedRecent = (): void => {
      if (cancelled) return;
      api.chat
        .getRecentMessages()
        .then((recent) => {
          if (!cancelled && recent.length > 0) addMessages(recent);
        })
        .catch((error) => logger.error('Failed to load recent chat history:', error));
    };
    seedRecent();
    // The api-client dispatches `backend:connected` / `backend:reconnected`
    // window events when the realtime socket (re)establishes.
    window.addEventListener('backend:connected', seedRecent);
    window.addEventListener('backend:reconnected', seedRecent);

    events
      .on<ChatMessage>(CHAT_MESSAGE_EVENT, (payload) => {
        // Plan B4 contract: MessageDeleted and UserBanned events mutate
        // PAST messages rather than appending a new row. Every other
        // ChatEvent variant flows through to the store: SubGifted / Raid /
        // MemberMilestone / etc. render as localized notice rows
        // (EventNotice), and RoomStateChanged feeds the ChannelModeBanner
        // (ChatList filters it out of the visible list).
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
      window.removeEventListener('backend:connected', seedRecent);
      window.removeEventListener('backend:reconnected', seedRecent);
      if (unlistenMessages) {
        unlistenMessages();
      }
      if (unlistenOverlay) {
        unlistenOverlay();
      }
    };
  }, [addMessage, addMessages, markMessageDeleted, markUserTimedOut, setOverlayTransparent]);
}
