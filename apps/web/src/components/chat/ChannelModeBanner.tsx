import type { ReactElement } from 'react';
import { useTranslation } from 'react-i18next';
import type { RoomMode } from '@/hooks/useRoomStateBanner';

function Chip({ label }: { label: string }): ReactElement {
  return (
    <span className="rounded-full border border-border-subtle bg-bg-elevated px-2 py-0.5 text-xs text-text-secondary">
      {label}
    </span>
  );
}

/**
 * Persistent channel-mode banner (Twitch ROOMSTATE), folded by
 * `useRoomStateBanner`. Renders nothing when no mode is active. Replaces the
 * blank "twitch" rows that ROOMSTATE used to produce in the message list.
 */
export function ChannelModeBanner({ mode }: { mode: RoomMode }): ReactElement | null {
  const { t } = useTranslation();
  const chips: string[] = [];

  if (mode.emoteOnly) chips.push(t('chat.roomMode.emoteOnly', { defaultValue: 'Emote-only' }));
  if (mode.subscribersOnly) {
    chips.push(t('chat.roomMode.subscribersOnly', { defaultValue: 'Subscribers-only' }));
  }
  if (mode.r9k) chips.push(t('chat.roomMode.r9k', { defaultValue: 'Unique-chat' }));
  if (mode.slowModeSecs > 0) {
    chips.push(
      t('chat.roomMode.slowMode', { defaultValue: 'Slow mode: {{secs}}s', secs: mode.slowModeSecs })
    );
  }
  if (mode.followersOnlyMinutes !== null) {
    chips.push(
      mode.followersOnlyMinutes > 0
        ? t('chat.roomMode.followersOnly', {
            defaultValue: 'Followers-only: {{minutes}}m',
            minutes: mode.followersOnlyMinutes,
          })
        : t('chat.roomMode.followersOnlyAny', { defaultValue: 'Followers-only' })
    );
  }

  if (chips.length === 0) return null;

  return (
    <div className="mt-3 flex flex-wrap items-center gap-1.5 shrink-0">
      {chips.map((label) => (
        <Chip key={label} label={label} />
      ))}
    </div>
  );
}
