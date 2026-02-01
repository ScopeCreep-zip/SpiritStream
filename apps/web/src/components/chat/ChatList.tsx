import { useEffect, useRef } from 'react';
import { cn } from '@/lib/cn';
import { PlatformIcon } from '@/components/stream/PlatformIcon';
import type { ChatMessage } from '@/types/chat';

export interface ChatListProps extends React.HTMLAttributes<HTMLDivElement> {
  messages: ChatMessage[];
  showEmptyState?: boolean;
  emptyLabel?: string;
  density?: 'default' | 'compact';
}

const densityStyles = {
  default: {
    rowGap: 'gap-3',
    text: 'text-sm',
  },
  compact: {
    rowGap: 'gap-2',
    text: 'text-xs',
  },
};

export function ChatList({
  messages,
  showEmptyState = true,
  emptyLabel = 'No chat messages yet.',
  density = 'default',
  className,
  ...props
}: ChatListProps) {
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (listRef.current) {
      listRef.current.scrollTop = listRef.current.scrollHeight;
    }
  }, [messages.length]);

  const densityConfig = densityStyles[density];

  return (
    <div ref={listRef} className={cn('overflow-y-auto', className)} {...props}>
      {messages.length === 0 ? (
        showEmptyState ? (
          <div
            className="text-center text-[var(--text-tertiary)]"
            style={{ padding: '40px 16px' }}
          >
            {emptyLabel}
          </div>
        ) : null
      ) : (
        <div className={cn('flex flex-col', densityConfig.rowGap)}>
          {messages.map((message) => (
            <div key={message.id} className={cn('flex items-start', densityConfig.rowGap)}>
              <PlatformIcon platform={message.platform} size="sm" />
              <div className={cn('flex flex-wrap items-baseline gap-x-2 gap-y-1', densityConfig.text)}>
                <span className="font-semibold text-[var(--text-primary)]">{message.username}</span>
                <span className="text-[var(--text-secondary)] break-words">{message.message}</span>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
