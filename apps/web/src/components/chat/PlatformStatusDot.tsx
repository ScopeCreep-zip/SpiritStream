import { useTranslation } from 'react-i18next';
import type { ChatPlatformStatus } from '@spiritstream/types';
import { cn } from '@/lib/cn';

/**
 * Shared status → color-token mapping for chat connection state.
 * Render-only: the status itself comes from `GET /chat/status` (via
 * `useChatPlatformStatus`); the frontend never decides what state a
 * platform is in, only how that state looks.
 */
export function statusDotClass(status: ChatPlatformStatus['status']): string {
  switch (status) {
    case 'connected':
      return 'bg-status-live';
    case 'connecting':
      return 'bg-status-connecting';
    case 'error':
      return 'bg-status-error';
    default:
      return 'bg-text-tertiary';
  }
}

interface PlatformConnectionBadgeProps {
  status: ChatPlatformStatus['status'];
}

/** Live connection pill for a chat-platform settings card. */
export function PlatformConnectionBadge({
  status,
}: PlatformConnectionBadgeProps): React.ReactElement {
  const { t } = useTranslation();
  let label: string;
  switch (status) {
    case 'connected':
      label = t('chat.status.connected', { defaultValue: 'Connected' });
      break;
    case 'connecting':
      label = t('chat.status.connecting', { defaultValue: 'Connecting…' });
      break;
    case 'error':
      label = t('chat.status.error', { defaultValue: 'Connection error' });
      break;
    default:
      label = t('chat.status.disconnected', { defaultValue: 'Not connected' });
      break;
  }
  return (
    <span
      className="inline-flex shrink-0 items-center gap-1.5 rounded-full border border-border-subtle bg-bg-elevated px-2 py-0.5 text-xs"
      role="status"
    >
      <span
        className={cn('inline-block h-2 w-2 rounded-full', statusDotClass(status))}
        aria-hidden="true"
      />
      <span className="text-text-secondary">{label}</span>
    </span>
  );
}
