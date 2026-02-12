import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/cn';
import type { ChatMessage, ChatPlatform } from '@/types/chat';

const CHAT_PLATFORM_STYLES: Record<ChatPlatform, { abbreviation: string; color: string; textColor: string }> = {
  twitch: { abbreviation: 'TW', color: '#9146FF', textColor: '#FFFFFF' },
  youtube: { abbreviation: 'YT', color: '#FF0000', textColor: '#FFFFFF' },
  trovo: { abbreviation: 'TR', color: '#1ECD97', textColor: '#000000' },
  stripchat: { abbreviation: 'SC', color: '#F97316', textColor: '#FFFFFF' },
  tiktok: { abbreviation: 'TK', color: '#000000', textColor: '#FFFFFF' },
  kick: { abbreviation: 'KK', color: '#53FC18', textColor: '#000000' },
  facebook: { abbreviation: 'FB', color: '#1877F2', textColor: '#FFFFFF' },
};

function ChatPlatformIcon({ platform, size = 'sm' }: { platform: string; size?: 'sm' | 'md' }) {
  const config = CHAT_PLATFORM_STYLES[platform as ChatPlatform] ?? {
    abbreviation: platform.slice(0, 2).toUpperCase(),
    color: '#6B7280',
    textColor: '#FFFFFF',
  };
  const sizeClass = size === 'sm' ? 'w-6 h-6 text-[0.625rem]' : 'w-8 h-8 text-xs';
  return (
    <div
      className={cn('rounded-md flex items-center justify-center font-semibold shrink-0', sizeClass)}
      style={{ backgroundColor: config.color, color: config.textColor }}
    >
      {config.abbreviation}
    </div>
  );
}

export interface ChatListProps extends React.HTMLAttributes<HTMLDivElement> {
  messages: ChatMessage[];
  showEmptyState?: boolean;
  emptyLabel?: string;
  density?: 'default' | 'compact';
  showTimestamps?: boolean;
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
  showTimestamps = false,
  className,
  ...props
}: ChatListProps) {
  const { t } = useTranslation();
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
          {messages.map((message) => {
            const platforms = message.platforms && message.platforms.length > 0
              ? message.platforms
              : [message.platform as ChatPlatform];
            const isOutbound = message.direction === 'outbound';
            const timestamp =
              showTimestamps && message.timestamp
                ? new Date(message.timestamp).toLocaleTimeString([], {
                    hour: '2-digit',
                    minute: '2-digit',
                  })
                : null;

            return (
              <div
                key={message.id}
                className={cn(
                  'flex items-start gap-3 rounded-lg border p-3',
                  isOutbound
                    ? 'border-[var(--border-strong)] bg-[var(--bg-base)]'
                    : 'border-[var(--border-subtle)] bg-[var(--bg-elevated)]'
                )}
              >
                <div className="flex items-center gap-1">
                  {platforms.map((platform) => (
                    <ChatPlatformIcon key={`${message.id}-${platform}`} platform={platform} />
                  ))}
                </div>
                <div className={cn('flex flex-wrap items-baseline gap-x-2 gap-y-1', densityConfig.text)}>
                  <span className="font-semibold text-[var(--text-primary)]">
                    {isOutbound ? t('chat.you') : message.username}
                  </span>
                  {timestamp && (
                    <span className="text-[0.7rem] text-[var(--text-tertiary)]">
                      {timestamp}
                    </span>
                  )}
                  <span className="text-[var(--text-secondary)] break-words">{message.message}</span>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
