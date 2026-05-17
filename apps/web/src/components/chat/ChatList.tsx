import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/cn';
import type { ChatMessage, ChatPlatform } from '@spiritstream/types';

// Platform → CSS-variable suffix. The colors themselves live in
// `tokens.css` (`--platform-X-bg` / `--platform-X-fg`).
// Anything unknown falls through to the `default` token pair.
const PLATFORM_ABBREVIATIONS: Record<ChatPlatform, string> = {
  twitch: 'TW',
  youtube: 'YT',
  trovo: 'TR',
  stripchat: 'SC',
  tiktok: 'TK',
  kick: 'KK',
  facebook: 'FB',
};

const KNOWN_PLATFORM_TOKENS = new Set<string>([
  'twitch',
  'youtube',
  'trovo',
  'stripchat',
  'tiktok',
  'kick',
  'facebook',
]);

function ChatPlatformIcon({ platform, size = 'sm' }: { platform: string; size?: 'sm' | 'md' }) {
  const abbreviation =
    PLATFORM_ABBREVIATIONS[platform as ChatPlatform] ?? platform.slice(0, 2).toUpperCase();
  const tokenKey = KNOWN_PLATFORM_TOKENS.has(platform) ? platform : 'default';
  const sizeClass = size === 'sm' ? 'w-6 h-6 text-[0.625rem]' : 'w-8 h-8 text-xs';
  return (
    <div
      className={cn(
        'platform-badge rounded-md flex items-center justify-center font-semibold shrink-0',
        sizeClass,
      )}
      data-platform={tokenKey}
    >
      {abbreviation}
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
            className="text-center text-text-tertiary py-10 px-4"
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
                ? new Date(Number(message.timestamp)).toLocaleTimeString([], {
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
                    ? 'border-border-strong bg-bg-base'
                    : 'border-border-subtle bg-bg-elevated'
                )}
              >
                <div className="flex items-center gap-1">
                  {platforms.map((platform) => (
                    <ChatPlatformIcon key={`${message.id}-${platform}`} platform={platform} />
                  ))}
                </div>
                <div className={cn('flex flex-wrap items-baseline gap-x-2 gap-y-1', densityConfig.text)}>
                  <span className="font-semibold text-text-primary">
                    {isOutbound ? t('chat.you') : message.username}
                  </span>
                  {timestamp && (
                    <span className="text-[0.7rem] text-text-tertiary">
                      {timestamp}
                    </span>
                  )}
                  <span className="text-text-secondary break-words">{message.message}</span>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
