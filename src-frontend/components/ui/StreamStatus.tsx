import { cn } from '@/lib/cn';
import type { StreamStatusType } from '@/types/stream';

export interface StreamStatusProps {
  status: StreamStatusType;
  label?: string;
  showPulse?: boolean;
  className?: string;
}

const statusConfig = {
  live: {
    bg: 'bg-[var(--status-live-bg)]',
    text: 'text-[var(--status-live-text)]',
    dot: 'bg-[var(--status-live)]',
    pulse: true,
    defaultLabel: 'Live',
  },
  connecting: {
    bg: 'bg-[var(--status-connecting-bg)]',
    text: 'text-[var(--status-connecting-text)]',
    dot: 'bg-[var(--status-connecting)]',
    pulse: true,
    defaultLabel: 'Connecting',
  },
  offline: {
    bg: 'bg-[var(--status-offline-bg)]',
    text: 'text-[var(--status-offline-text)]',
    dot: 'bg-[var(--status-offline)]',
    pulse: false,
    defaultLabel: 'Offline',
  },
  error: {
    bg: 'bg-[var(--error-subtle)]',
    text: 'text-[var(--error-text)]',
    dot: 'bg-[var(--error)]',
    pulse: false,
    defaultLabel: 'Error',
  },
};

export function StreamStatus({
  status,
  label,
  showPulse = true,
  className,
}: StreamStatusProps) {
  const config = statusConfig[status];
  const shouldPulse = showPulse && config.pulse;

  return (
    <span
      className={cn(
        'inline-flex items-center gap-1.5 rounded-full text-xs font-medium',
        config.bg,
        config.text,
        className
      )}
      style={{ padding: '4px 10px' }}
    >
      <span
        className={cn(
          'w-1.5 h-1.5 rounded-full',
          config.dot,
          shouldPulse && 'animate-pulse'
        )}
      />
      {label || config.defaultLabel}
    </span>
  );
}
