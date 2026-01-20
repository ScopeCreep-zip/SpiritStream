import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/cn';
import type { StreamStatusType } from '@/types/stream';

export interface StreamStatusProps {
  status: StreamStatusType;
  label?: string;
  showPulse?: boolean;
  className?: string;
}

const statusStyles = {
  live: {
    bg: 'bg-[var(--status-live-bg)]',
    text: 'text-[var(--status-live-text)]',
    dot: 'bg-[var(--status-live)]',
    pulse: true,
  },
  connecting: {
    bg: 'bg-[var(--status-connecting-bg)]',
    text: 'text-[var(--status-connecting-text)]',
    dot: 'bg-[var(--status-connecting)]',
    pulse: true,
  },
  offline: {
    bg: 'bg-[var(--status-offline-bg)]',
    text: 'text-[var(--status-offline-text)]',
    dot: 'bg-[var(--status-offline)]',
    pulse: false,
  },
  error: {
    bg: 'bg-[var(--error-subtle)]',
    text: 'text-[var(--error-text)]',
    dot: 'bg-[var(--error)]',
    pulse: false,
  },
};

export function StreamStatus({ status, label, showPulse = true, className }: StreamStatusProps) {
  const { t } = useTranslation();
  const styles = statusStyles[status];
  const shouldPulse = showPulse && styles.pulse;

  // Get translated default label based on status
  const defaultLabels: Record<StreamStatusType, string> = {
    live: t('status.live'),
    connecting: t('status.connecting'),
    offline: t('status.offline'),
    error: t('status.error'),
  };

  return (
    <span
      className={cn(
        'inline-flex items-center gap-1.5 rounded-full text-xs font-medium',
        styles.bg,
        styles.text,
        className
      )}
      style={{ padding: '4px 10px' }}
    >
      <span
        className={cn('w-1.5 h-1.5 rounded-full', styles.dot, shouldPulse && 'animate-pulse')}
      />
      {label || defaultLabels[status]}
    </span>
  );
}
