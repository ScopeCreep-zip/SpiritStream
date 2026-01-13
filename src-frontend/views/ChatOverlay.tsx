import { useEffect } from 'react';
import { cn } from '@/lib/cn';
import { ChatList } from '@/components/chat/ChatList';
import { useChatStore } from '@/stores/chatStore';

export function ChatOverlay() {
  const messages = useChatStore((state) => state.messages);
  const overlayTransparent = useChatStore((state) => state.overlayTransparent);

  useEffect(() => {
    if (typeof document === 'undefined') return;
    document.body.dataset.window = 'chat-overlay';
    if (overlayTransparent) {
      document.body.dataset.overlayTransparent = 'true';
    } else {
      delete document.body.dataset.overlayTransparent;
    }

    return () => {
      delete document.body.dataset.window;
      delete document.body.dataset.overlayTransparent;
    };
  }, [overlayTransparent]);

  return (
    <div
      className={cn(
        'min-h-screen w-full',
        overlayTransparent ? 'bg-transparent' : 'bg-[var(--bg-base)]'
      )}
    >
      <ChatList
        messages={messages}
        showEmptyState={false}
        density="compact"
        className="h-screen px-4 py-6"
      />
    </div>
  );
}
