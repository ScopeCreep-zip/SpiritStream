import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/cn';
import { hasFlag, MessageFlag } from '@/lib/messageFlags';
import type { ChatMessage, ChatPlatform } from '@spiritstream/types';
import { Fragment } from './Fragment';

// Platform → CSS-variable suffix. The colors themselves live in
// `tokens.css` (`--platform-X-bg` / `--platform-X-fg`).
// Anything unknown falls through to the `default` token pair.
const PLATFORM_ABBREVIATIONS: Record<ChatPlatform, string> = {
  twitch: 'TW',
  youtube: 'YT',
  trovo: 'TR',
  tiktok: 'TK',
  kick: 'KK',
  facebook: 'FB',
};

const KNOWN_PLATFORM_TOKENS = new Set<string>([
  'twitch',
  'youtube',
  'trovo',
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
        sizeClass
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

  const renderEmpty = showEmptyState ? (
    <div className="text-center text-text-tertiary py-10 px-4">{emptyLabel}</div>
  ) : null;

  return (
    <div ref={listRef} className={cn('overflow-y-auto', className)} {...props}>
      {messages.length === 0 ? (
        renderEmpty
      ) : (
        <div className={cn('flex flex-col', densityConfig.rowGap)}>
          {messages.map((message) => {
            const platforms =
              message.platforms && message.platforms.length > 0
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

            // Phase B contract: when `fragments` is populated, the
            // renderer reads it verbatim. Empty `fragments` means a
            // legacy log-JSONL entry (pre-Phase-B); fall back to
            // rendering `message.message` as plain text so the chat
            // log replay still works.
            const flags = message.flags ?? 0;
            const isDisabled = hasFlag(flags, MessageFlag.DISABLED);
            const isAction = hasFlag(flags, MessageFlag.ACTION);
            const isHighlighted = hasFlag(flags, MessageFlag.HIGHLIGHTED);
            const isFirstMessage = hasFlag(flags, MessageFlag.FIRST_MESSAGE);
            const isElevated = hasFlag(flags, MessageFlag.ELEVATED_MESSAGE);
            const isSystem = hasFlag(flags, MessageFlag.SYSTEM);
            const isTimedOutAuthor = hasFlag(flags, MessageFlag.TIMED_OUT_AUTHOR);

            const highlightStyle: React.CSSProperties | undefined = message.highlightColor
              ? { backgroundColor: message.highlightColor.hex }
              : undefined;

            return (
              <div
                key={message.id}
                className={cn(
                  'flex items-start gap-3 rounded-lg border p-3',
                  isOutbound
                    ? 'border-border-strong bg-bg-base'
                    : 'border-border-subtle bg-bg-elevated',
                  isDisabled && 'opacity-50 line-through',
                  isAction && 'italic',
                  isHighlighted && 'ring-1 ring-purple-violet-500',
                  isFirstMessage && 'border-l-4 border-l-purple-violet-500',
                  isElevated && 'border-l-4 border-l-fuchsia-500',
                  isSystem && 'bg-bg-elevated/50',
                  isTimedOutAuthor && 'opacity-60'
                )}
                style={highlightStyle}
                data-flags={flags}
              >
                <div className="flex items-center gap-1">
                  {platforms.map((platform) => (
                    <ChatPlatformIcon key={`${message.id}-${platform}`} platform={platform} />
                  ))}
                </div>
                <div
                  className={cn(
                    'flex flex-wrap items-baseline gap-x-2 gap-y-1',
                    densityConfig.text
                  )}
                >
                  <span
                    className="font-semibold text-text-primary"
                    style={message.author?.color ? { color: message.author.color.hex } : undefined}
                  >
                    {isOutbound ? t('chat.you') : (message.author?.displayName ?? message.username)}
                  </span>
                  {timestamp && (
                    <span className="text-[0.7rem] text-text-tertiary">{timestamp}</span>
                  )}
                  {message.fragments && message.fragments.length > 0 ? (
                    <span className="text-text-secondary break-words inline-flex flex-wrap items-baseline gap-x-1">
                      {message.fragments.map((fragment, i) => (
                        <Fragment key={i} fragment={fragment} />
                      ))}
                    </span>
                  ) : (
                    <span className="text-text-secondary break-words">{message.message}</span>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
