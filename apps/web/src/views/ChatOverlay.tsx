import { useEffect, useState } from 'react';
import { X } from 'lucide-react';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { listen } from '@tauri-apps/api/event';
import { cn } from '@/lib/cn';
import { ChatList } from '@/components/chat/ChatList';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { CHAT_OVERLAY_SETTINGS_EVENT, CHAT_OVERLAY_ALWAYS_ON_TOP_EVENT } from '@/lib/chatEvents';
import { isTauri } from '@/lib/backend/env';
import { setupOverlayAutoClose } from '@/lib/chatWindow';
import { useChatStore } from '@/stores/chatStore';

export function ChatOverlay() {
  const messages = useChatStore((state) => state.messages);
  const overlayTransparent = useChatStore((state) => state.overlayTransparent);
  const setOverlayTransparent = useChatStore((state) => state.setOverlayTransparent);
  const [draftMessage, setDraftMessage] = useState('');

  // Set up auto-close when main window closes
  useEffect(() => {
    setupOverlayAutoClose();
  }, []);

  // Listen for settings changes from the main window
  useEffect(() => {
    if (!isTauri()) return;

    let unlistenTransparent: (() => void) | undefined;
    let unlistenAlwaysOnTop: (() => void) | undefined;

    listen<{ transparent: boolean }>(CHAT_OVERLAY_SETTINGS_EVENT, (event) => {
      setOverlayTransparent(event.payload.transparent);
    }).then((fn) => {
      unlistenTransparent = fn;
    });

    listen<{ alwaysOnTop: boolean }>(CHAT_OVERLAY_ALWAYS_ON_TOP_EVENT, async (event) => {
      try {
        const currentWindow = getCurrentWindow();
        await currentWindow.setAlwaysOnTop(event.payload.alwaysOnTop);
      } catch (error) {
        console.error('Failed to set always on top:', error);
      }
    }).then((fn) => {
      unlistenAlwaysOnTop = fn;
    });

    return () => {
      unlistenTransparent?.();
      unlistenAlwaysOnTop?.();
    };
  }, [setOverlayTransparent]);

  useEffect(() => {
    if (typeof document === 'undefined') return;

    // Set on both html and body for CSS targeting
    const html = document.documentElement;
    const body = document.body;

    html.dataset.window = 'chat-overlay';
    body.dataset.window = 'chat-overlay';

    if (overlayTransparent) {
      html.dataset.overlayTransparent = 'true';
      body.dataset.overlayTransparent = 'true';
    } else {
      delete html.dataset.overlayTransparent;
      delete body.dataset.overlayTransparent;
    }

    return () => {
      delete html.dataset.window;
      delete html.dataset.overlayTransparent;
      delete body.dataset.window;
      delete body.dataset.overlayTransparent;
    };
  }, [overlayTransparent]);

  const handleClose = async () => {
    try {
      await WebviewWindow.getCurrent().close();
    } catch (error) {
      console.error('Failed to close chat overlay window:', error);
    }
  };

  const handleDragStart = () => {
    getCurrentWindow().startDragging().catch((error) => {
      console.error('Failed to start dragging chat overlay window:', error);
    });
  };

  const handleSend = () => {
    const trimmed = draftMessage.trim();
    if (!trimmed) return;
    console.info('[ChatOverlay] Send not wired yet:', trimmed);
    setDraftMessage('');
  };

  return (
    <div
      className={cn(
        'min-h-screen w-full flex flex-col',
        overlayTransparent ? 'bg-transparent' : 'bg-[var(--bg-base)]'
      )}
    >
      <div
        className="flex items-center justify-end px-10 pt-4 pb-2 cursor-grab active:cursor-grabbing"
        data-tauri-drag-region
        onPointerDown={handleDragStart}
      >
        <Button
          variant="ghost"
          size="icon"
          onClick={handleClose}
          aria-label="Close chat overlay"
          data-tauri-drag-region="false"
          onPointerDown={(event) => event.stopPropagation()}
        >
          <X className="w-4 h-4" />
        </Button>
      </div>
      <div className="flex-1 min-h-0 px-10">
        <ChatList
          messages={messages}
          showEmptyState={false}
          density="compact"
          className="h-full pb-4"
        />
      </div>
      <form
        className="flex items-center gap-3 px-10 pb-10 pt-3"
        onSubmit={(event) => {
          event.preventDefault();
          handleSend();
        }}
      >
        <div className="flex-1 min-w-0">
          <Input
            value={draftMessage}
            onChange={(event) => setDraftMessage(event.target.value)}
            placeholder="Type a message..."
            aria-label="Chat message"
            className="w-full"
          />
        </div>
        <Button
          type="submit"
          size="sm"
          disabled={!draftMessage.trim()}
          className="w-24 shrink-0"
        >
          Send
        </Button>
      </form>
    </div>
  );
}
