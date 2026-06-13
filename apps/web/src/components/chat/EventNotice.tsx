import type { ReactElement, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Award,
  Ban,
  Coins,
  Crown,
  Gift,
  Info,
  Megaphone,
  Sparkles,
  Vote,
} from 'lucide-react';
import type { ChatMessage } from '@spiritstream/types';

/** Micros → localized currency string. Presentation only (same category as
 *  `toLocaleTimeString` in ChatList) — no business logic. */
function formatAmount(amountMicros: number, currency: string): string {
  const value = amountMicros / 1_000_000;
  try {
    return new Intl.NumberFormat(undefined, { style: 'currency', currency }).format(value);
  } catch {
    return `${value.toFixed(2)} ${currency}`;
  }
}

function NoticeRow({ icon, children }: { icon: ReactElement; children: ReactNode }): ReactElement {
  return (
    <div className="flex items-center gap-2 rounded-lg border border-border-subtle bg-bg-elevated/50 px-3 py-1.5 text-xs text-text-secondary">
      <span className="text-text-tertiary">{icon}</span>
      <span className="break-words">{children}</span>
    </div>
  );
}

/**
 * Renders a non-text `ChatEvent` (sub, raid, cheer, mode change, …) as a
 * localized notice row, distinct from chat messages. Mirrors `Fragment.tsx`'s
 * discriminated-union `switch` over the `kind` tag. Events whose actor is the
 * message author (cheer, super chat, milestone) read the name from
 * `message.author`; the rest carry it in the payload.
 *
 * `messageDeleted` / `userBanned` are intercepted upstream
 * (`useChatListener`) for live events and mutate past rows instead, so they
 * only reach here on history replay; `roomStateChanged` feeds the
 * `ChannelModeBanner` and is filtered out before this renders.
 */
export function EventNotice({ message }: { message: ChatMessage }): ReactElement | null {
  const { t } = useTranslation();
  const event = message.event;
  if (!event) return null;
  const name = message.author?.displayName ?? message.username;

  switch (event.kind) {
    case 'raid':
      return (
        <NoticeRow icon={<Megaphone className="w-3.5 h-3.5" />}>
          {t('chat.events.raid', {
            defaultValue: '{{name}} raided with {{count}} viewers',
            name: event.raiderDisplayName,
            count: event.viewerCount,
          })}
        </NoticeRow>
      );
    case 'subGifted':
      return (
        <NoticeRow icon={<Gift className="w-3.5 h-3.5" />}>
          {t('chat.events.subGifted', {
            defaultValue: '{{name}} gifted {{count}} sub(s)',
            name: event.gifterDisplayName,
            count: event.count,
          })}
        </NoticeRow>
      );
    case 'cheer':
      return (
        <NoticeRow icon={<Coins className="w-3.5 h-3.5" />}>
          {t('chat.events.cheer', {
            defaultValue: '{{name}} cheered {{bits}} bits',
            name,
            bits: event.bits,
          })}
        </NoticeRow>
      );
    case 'memberMilestone':
      return (
        <NoticeRow icon={<Award className="w-3.5 h-3.5" />}>
          {t('chat.events.memberMilestone', {
            defaultValue: '{{name}} has been a member for {{months}} months',
            name: event.displayName,
            months: event.months,
          })}
        </NoticeRow>
      );
    case 'newSponsor':
      return (
        <NoticeRow icon={<Crown className="w-3.5 h-3.5" />}>
          {t('chat.events.newSponsor', {
            defaultValue: '{{name}} joined as a {{tier}} member',
            name: event.sponsorDisplayName,
            tier: event.tierName,
          })}
        </NoticeRow>
      );
    case 'superChat':
      return (
        <NoticeRow icon={<Sparkles className="w-3.5 h-3.5" />}>
          {t('chat.events.superChat', {
            defaultValue: '{{name}} sent a Super Chat ({{amount}})',
            name,
            amount: formatAmount(event.amountMicros, event.currency),
          })}
        </NoticeRow>
      );
    case 'superSticker':
      return (
        <NoticeRow icon={<Sparkles className="w-3.5 h-3.5" />}>
          {t('chat.events.superSticker', {
            defaultValue: '{{name}} sent a Super Sticker ({{amount}})',
            name,
            amount: formatAmount(event.amountMicros, event.currency),
          })}
        </NoticeRow>
      );
    case 'hypeChat':
      return (
        <NoticeRow icon={<Sparkles className="w-3.5 h-3.5" />}>
          {t('chat.events.hypeChat', {
            defaultValue: '{{name}} sent a Hype Chat ({{amount}})',
            name,
            amount: formatAmount(event.amountMicros, event.currency),
          })}
        </NoticeRow>
      );
    case 'poll':
      return (
        <NoticeRow icon={<Vote className="w-3.5 h-3.5" />}>
          {t('chat.events.poll', { defaultValue: 'Poll: {{question}}', question: event.question })}
        </NoticeRow>
      );
    case 'chatEnded':
      return (
        <NoticeRow icon={<Info className="w-3.5 h-3.5" />}>
          {t('chat.events.chatEnded', { defaultValue: 'Chat has ended.' })}
        </NoticeRow>
      );
    case 'userBanned':
      // Only reached on history replay (live events mutate past rows).
      return (
        <NoticeRow icon={<Ban className="w-3.5 h-3.5" />}>
          {event.durationSecs == null
            ? t('chat.events.userBanned', {
                defaultValue: '{{name}} was banned',
                name: event.userLogin,
              })
            : t('chat.events.userTimedOut', {
                defaultValue: '{{name}} was timed out for {{secs}}s',
                name: event.userLogin,
                secs: event.durationSecs,
              })}
        </NoticeRow>
      );
    case 'messageDeleted':
    case 'roomStateChanged':
    case 'tombstone':
      return null;
    default: {
      const _exhaustive: never = event;
      return _exhaustive;
    }
  }
}
