import { useEffect } from 'react';
import { events, parseChatMessage } from '@spiritstream/api-client';
import { api } from '@/lib/client';
import { logger } from '@/lib/logger';
import { useChatStore } from '@/stores/chatStore';
import type { ChatMessage } from '@spiritstream/types';
import {
  CHAT_AUTO_CONNECTED_EVENT,
  CHAT_AUTO_CONNECT_FAILED_EVENT,
  CHAT_MESSAGE_EVENT,
  CHAT_OVERLAY_SETTINGS_EVENT,
} from '@/lib/chatEvents';
import { toast } from '@/hooks/useToast';
import i18n from '@/lib/i18n';

type UnlistenFn = () => void;

export function useChatListener() {
  const addMessage = useChatStore((state) => state.addMessage);
  const addMessages = useChatStore((state) => state.addMessages);
  const markMessageDeleted = useChatStore((state) => state.markMessageDeleted);
  const markUserTimedOut = useChatStore((state) => state.markUserTimedOut);
  const setOverlayOpacity = useChatStore((state) => state.setOverlayOpacity);

  useEffect(() => {
    // Unmount can race the async registrations; the cancelled flag makes
    // a late-resolving subscribe release itself instead of leaking.
    let cancelled = false;
    let unlistenMessages: UnlistenFn | null = null;
    let unlistenOverlay: UnlistenFn | null = null;
    let unlistenConnected: UnlistenFn | null = null;
    let unlistenConnectFailed: UnlistenFn | null = null;

    const platformLabel = (platform: string): string =>
      i18n.t(`chat.platforms.${platform}`, { defaultValue: platform });

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
      .on<ChatMessage>(CHAT_MESSAGE_EVENT, (raw) => {
        // Untrusted inbound: live chat messages originate from arbitrary
        // third parties on the streaming platforms. Validate the scalar
        // fields at runtime before anything renders them; a malformed /
        // hostile payload is dropped (logged) rather than reaching the UI.
        const payload = parseChatMessage(raw);
        if (!payload) return;
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
      .on<{ opacity?: number }>(CHAT_OVERLAY_SETTINGS_EVENT, (payload) => {
        if (typeof payload.opacity === 'number') {
          setOverlayOpacity(payload.opacity);
        }
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

    // Connect outcomes were emitted by the backend but nothing listened, so a
    // failed Connect (e.g. YouTube with no live broadcast) silently did
    // nothing. Surface both as toasts so the user always gets an answer.
    events
      .on<{ platform: string }>(CHAT_AUTO_CONNECTED_EVENT, (payload) => {
        toast.success(
          i18n.t('chat.connect.connected', {
            defaultValue: '{{platform}} chat connected.',
            platform: platformLabel(payload.platform),
          })
        );
      })
      .then((unsubscribe) => {
        if (cancelled) {
          unsubscribe();
          return;
        }
        unlistenConnected = unsubscribe;
      })
      .catch((error) => {
        logger.error('Failed to listen for chat connect events:', error);
      });

    events
      .on<{ platform: string; kind?: string; error?: string }>(
        CHAT_AUTO_CONNECT_FAILED_EVENT,
        (payload) => {
          toast.error(
            i18n.t('chat.connect.connectFailed', {
              defaultValue: 'Could not connect {{platform}} chat: {{error}}',
              platform: platformLabel(payload.platform),
              error:
                payload.error ??
                i18n.t('chat.connect.unknownError', { defaultValue: 'unknown error' }),
            })
          );
        }
      )
      .then((unsubscribe) => {
        if (cancelled) {
          unsubscribe();
          return;
        }
        unlistenConnectFailed = unsubscribe;
      })
      .catch((error) => {
        logger.error('Failed to listen for chat connect-failed events:', error);
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
      if (unlistenConnected) {
        unlistenConnected();
      }
      if (unlistenConnectFailed) {
        unlistenConnectFailed();
      }
    };
  }, [addMessage, addMessages, markMessageDeleted, markUserTimedOut, setOverlayOpacity]);
}
