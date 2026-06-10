import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/cn';
import { isTauri } from '@spiritstream/api-client';
import {
  useConnectionStore,
  type ConnectionStatus as ConnectionStatusType,
} from '@/stores/connectionStore';

/**
 * Browser-only WebSocket connection indicator. Renders nothing in the
 * Tauri desktop shell because:
 *   1. The shell co-spawns the backend sidecar on localhost — connection
 *      failure means the entire shell is broken, not a soft state worth
 *      a corner badge.
 *   2. The shell's `wait_for_tcp_listening` already gates the webview
 *      from existing before the backend is reachable.
 *
 * In browser / Docker, the WebSocket runs over a real network and can
 * legitimately drop. This badge tells the user whether server-pushed
 * updates (chat messages, stream stats, OBS state) are still live.
 *
 * Strictly single-purpose: this badge reflects ONLY the
 * `ws://.../api/v1/events` socket state from `connectionStore`. Other
 * error surfaces (toasts, `ConnectionError` overlay, `ErrorBoundary`)
 * handle their own concerns.
 */
export interface ConnectionStatusProps {
  className?: string;
}

interface DotConfig {
  /** Tailwind class for the colored dot. */
  dotClass: string;
  /** Tailwind class for the label text. */
  textClass: string;
  /** Whether the dot should pulse — used for in-flight states. */
  animate: boolean;
}

const DOT_CONFIG: Record<ConnectionStatusType, DotConfig> = {
  connected: {
    dotClass: 'bg-success',
    textClass: 'text-text-secondary',
    animate: false,
  },
  connecting: {
    dotClass: 'bg-warning',
    textClass: 'text-text-secondary',
    animate: true,
  },
  disconnected: {
    dotClass: 'bg-error',
    textClass: 'text-error-text',
    animate: false,
  },
};

export function ConnectionStatus({ className }: ConnectionStatusProps) {
  const { t } = useTranslation();
  const { status, lastConnected } = useConnectionStore();

  // Tauri shell guarantees backend reachability; the badge has no signal
  // there and would only add visual noise.
  if (isTauri()) {
    return null;
  }

  const config = DOT_CONFIG[status];

  // "Connecting" vs "Reconnecting" is decided by whether we've ever
  // connected before — `lastConnected` is null until the first
  // successful connect, then non-null forever. Avoids the prior bug
  // where a per-attempt counter mis-classified the first connect as a
  // reconnect (which produced a spurious "reconnecting…" label flash).
  let label: string;
  if (status === 'connected') {
    label = t('connection.serverBadge.connected');
  } else if (status === 'disconnected') {
    label = t('connection.serverBadge.disconnected');
  } else if (lastConnected !== null) {
    label = t('connection.serverBadge.reconnecting');
  } else {
    label = t('connection.serverBadge.connecting');
  }

  return (
    <div
      className={cn(
        'inline-flex items-center gap-2 text-xs font-medium',
        config.textClass,
        className
      )}
      role="status"
      aria-live="polite"
    >
      <span
        className={cn(
          'w-2 h-2 rounded-full shrink-0',
          config.dotClass,
          config.animate && 'animate-pulse'
        )}
        aria-hidden="true"
      />
      <span>{label}</span>
    </div>
  );
}
