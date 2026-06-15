import { useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { emit, listen } from '@tauri-apps/api/event';
import { ChatList } from '@/components/chat/ChatList';
import {
  CHAT_OVERLAY_SETTINGS_EVENT,
  CHAT_OVERLAY_ALWAYS_ON_TOP_EVENT,
  CHAT_OVERLAY_SYNC_EVENT,
  CHAT_OVERLAY_SYNC_REQUEST_EVENT,
} from '@/lib/chatEvents';
import { isTauri } from '@spiritstream/api-client';
import { logger } from '@/lib/logger';
import { setupOverlayAutoClose } from '@/lib/chatWindow';
import { useChatStore } from '@/stores/chatStore';
import type { ChatMessage } from '@spiritstream/types';

export function ChatOverlay() {
  const messages = useChatStore((state) => state.messages);
  const overlayOpacity = useChatStore((state) => state.overlayOpacity);
  const setOverlayOpacity = useChatStore((state) => state.setOverlayOpacity);
  const addMessages = useChatStore((state) => state.addMessages);

  // Set up auto-close when main window closes
  useEffect(() => {
    setupOverlayAutoClose();
  }, []);

  // Listen for settings changes from the main window
  useEffect(() => {
    if (!isTauri()) return;

    let unlistenTransparent: (() => void) | undefined;
    let unlistenAlwaysOnTop: (() => void) | undefined;
    let unlistenSync: (() => void) | undefined;

    listen<{ opacity: number }>(CHAT_OVERLAY_SETTINGS_EVENT, (event) => {
      if (typeof event.payload.opacity === 'number') {
        setOverlayOpacity(event.payload.opacity);
      }
    }).then((fn) => {
      unlistenTransparent = fn;
    });

    listen<{ alwaysOnTop: boolean }>(CHAT_OVERLAY_ALWAYS_ON_TOP_EVENT, async (event) => {
      try {
        const currentWindow = getCurrentWindow();
        await currentWindow.setAlwaysOnTop(event.payload.alwaysOnTop);
      } catch (error) {
        logger.error('Failed to set always on top:', error);
      }
    }).then((fn) => {
      unlistenAlwaysOnTop = fn;
    });

    listen<{ messages: ChatMessage[] }>(CHAT_OVERLAY_SYNC_EVENT, (event) => {
      if (event.payload?.messages?.length) {
        addMessages(event.payload.messages);
      }
    }).then((fn) => {
      unlistenSync = fn;
    });

    return () => {
      unlistenTransparent?.();
      unlistenAlwaysOnTop?.();
      unlistenSync?.();
    };
  }, [addMessages, setOverlayOpacity]);

  useEffect(() => {
    if (isTauri()) {
      emit(CHAT_OVERLAY_SYNC_REQUEST_EVENT, {}).catch((error) => {
        logger.error('Failed to request chat overlay sync:', error);
      });
      return;
    }

    if (!window.opener) return;

    const handleMessage = (event: MessageEvent) => {
      // Reject messages from other origins — in browser mode (Docker
      // self-host, dev preview, etc.) an iframe attacker on a different
      // origin can forge sync messages and inject arbitrary "chat
      // messages" into the overlay. Tauri webview origin matches
      // `window.location.origin` for the bundled app shell.
      if (event.origin !== window.location.origin) return;
      if (!event.data || event.data.type !== 'chat-overlay-sync') return;
      const payload = event.data as { messages?: ChatMessage[] };
      if (payload.messages?.length) {
        addMessages(payload.messages);
      }
    };

    window.addEventListener('message', handleMessage);
    window.opener.postMessage({ type: 'chat-overlay-sync-request' }, window.location.origin);

    return () => {
      window.removeEventListener('message', handleMessage);
    };
  }, [addMessages]);

  useEffect(() => {
    if (typeof document === 'undefined') return;

    // Set on both html and body for CSS targeting
    const html = document.documentElement;
    const body = document.body;

    html.dataset.window = 'chat-overlay';
    body.dataset.window = 'chat-overlay';

    return () => {
      delete html.dataset.window;
      delete body.dataset.window;
    };
  }, []);

  // Overlay opacity → CSS var consumed by the normal background layer.
  useEffect(() => {
    if (typeof document === 'undefined') return;
    document.documentElement.style.setProperty('--overlay-opacity', String(overlayOpacity));
    return () => {
      document.documentElement.style.removeProperty('--overlay-opacity');
    };
  }, [overlayOpacity]);

  const handleDragStart = () => {
    getCurrentWindow()
      .startDragging()
      .catch((error) => {
        logger.error('Failed to start dragging chat overlay window:', error);
      });
  };

  // `h-screen` (definite height), NOT `min-h-screen` — the flex chain needs a
  // bounded height so ChatList scrolls INTERNALLY (header fixed,
  // auto-scroll-to-newest works) instead of growing past the window.
  // `chat-overlay-normal` paints a flat bg at the opacity slider.
  return (
    <div className="h-screen w-full flex flex-col chat-overlay-normal">
      {/* Slim grab strip — no close button (pop-out / collapse is owned by the
          console). Keeps the window draggable. */}
      <div
        className="h-7 w-full flex-shrink-0 cursor-grab active:cursor-grabbing"
        data-tauri-drag-region
        onPointerDown={handleDragStart}
      />
      <div className="flex-1 min-h-0 px-6 pb-6">
        <ChatList messages={messages} showEmptyState={false} density="compact" className="h-full" />
      </div>
    </div>
  );
}
