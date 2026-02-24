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
    bg: 'bg-status-live-bg',
    text: 'text-status-live-text',
    dot: 'bg-status-live',
    pulse: true,
  },
  connecting: {
    bg: 'bg-status-connecting-bg',
    text: 'text-status-connecting-text',
    dot: 'bg-status-connecting',
    pulse: true,
  },
  offline: {
    bg: 'bg-status-offline-bg',
    text: 'text-status-offline-text',
    dot: 'bg-status-offline',
    pulse: false,
  },
  error: {
    bg: 'bg-error-subtle',
    text: 'text-error-text',
    dot: 'bg-error',
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
        'inline-flex items-center gap-1.5 rounded-full text-xs font-medium py-1 px-2.5',
        styles.bg,
        styles.text,
        className
      )}
    >
      <span
        className={cn('w-1.5 h-1.5 rounded-full', styles.dot, shouldPulse && 'animate-pulse')}
      />
      {label || defaultLabels[status]}
    </span>
  );
}
