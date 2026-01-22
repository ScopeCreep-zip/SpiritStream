import { useTranslation } from 'react-i18next';
import { Wifi, WifiOff, Loader2 } from 'lucide-react';
import { cn } from '@/lib/cn';
import { useConnectionStore, type ConnectionStatus as ConnectionStatusType } from '@/stores/connectionStore';
import { backendMode } from '@/lib/backend/env';

export interface ConnectionStatusProps {
  className?: string;
  showLabel?: boolean;
}

const statusConfig: Record<
  ConnectionStatusType,
  {
    icon: typeof Wifi;
    bgClass: string;
    textClass: string;
    dotClass: string;
    animate?: boolean;
  }
> = {
  connected: {
    icon: Wifi,
    bgClass: 'bg-[var(--success-subtle)]',
    textClass: 'text-[var(--success-text)]',
    dotClass: 'bg-[var(--success)]',
  },
  connecting: {
    icon: Loader2,
    bgClass: 'bg-[var(--warning-subtle)]',
    textClass: 'text-[var(--warning-text)]',
    dotClass: 'bg-[var(--warning)]',
    animate: true,
  },
  disconnected: {
    icon: WifiOff,
    bgClass: 'bg-[var(--error-subtle)]',
    textClass: 'text-[var(--error-text)]',
    dotClass: 'bg-[var(--error)]',
  },
};

export function ConnectionStatus({ className, showLabel = true }: ConnectionStatusProps) {
  const { t } = useTranslation();
  const { status, reconnectAttempts } = useConnectionStore();

  // In Tauri mode, always show as connected (local IPC)
  const effectiveStatus = backendMode === 'tauri' ? 'connected' : status;
  const config = statusConfig[effectiveStatus];
  const Icon = config.icon;

  // Don't show anything in Tauri mode unless user explicitly wants it
  if (backendMode === 'tauri') {
    return null;
  }

  const labels: Record<ConnectionStatusType, string> = {
    connected: t('connection.connected', 'Connected'),
    connecting:
      reconnectAttempts > 0
        ? t('connection.reconnecting', 'Reconnecting...')
        : t('connection.connecting', 'Connecting...'),
    disconnected: t('connection.disconnected', 'Disconnected'),
  };

  return (
    <div
      className={cn(
        'inline-flex items-center gap-2 px-3 py-2 rounded-lg text-xs font-medium',
        config.bgClass,
        config.textClass,
        className
      )}
    >
      <Icon className={cn('w-4 h-4', config.animate && 'animate-spin')} />
      {showLabel && <span>{labels[effectiveStatus]}</span>}
    </div>
  );
}

/**
 * Compact connection indicator - just a dot with tooltip
 */
export function ConnectionIndicator({ className }: { className?: string }) {
  const { t } = useTranslation();
  const { status, reconnectAttempts } = useConnectionStore();

  // In Tauri mode, don't show indicator
  if (backendMode === 'tauri') {
    return null;
  }

  const config = statusConfig[status];

  const labels: Record<ConnectionStatusType, string> = {
    connected: t('connection.connected', 'Connected'),
    connecting:
      reconnectAttempts > 0
        ? t('connection.reconnecting', 'Reconnecting...')
        : t('connection.connecting', 'Connecting...'),
    disconnected: t('connection.disconnected', 'Disconnected'),
  };

  return (
    <div
      className={cn('flex items-center gap-2', className)}
      title={labels[status]}
    >
      <span
        className={cn(
          'w-2 h-2 rounded-full',
          config.dotClass,
          status === 'connecting' && 'animate-pulse'
        )}
      />
    </div>
  );
}
