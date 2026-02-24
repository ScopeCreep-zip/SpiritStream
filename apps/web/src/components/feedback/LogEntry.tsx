import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/cn';
import type { LogLevel } from '@/types/stream';

export interface LogEntryProps {
  time: string;
  level: LogLevel;
  message: string;
}

const levelStyles: Record<LogLevel, string> = {
  info: 'text-primary',
  warn: 'text-warning-text',
  error: 'text-error-text',
  debug: 'text-text-tertiary',
};

export function LogEntry({ time, level, message }: LogEntryProps) {
  const { t } = useTranslation();
  const levelLabels: Record<LogLevel, string> = {
    info: t('logs.info'),
    warn: t('logs.warning'),
    error: t('logs.error'),
    debug: t('logs.debug'),
  };

  return (
    <div
      className="flex border-b border-border-muted last:border-b-0 py-1.5 px-3 gap-3"
    >
      <span className="text-text-muted whitespace-nowrap">{time}</span>
      <span className={cn('font-semibold w-12', levelStyles[level])}>{levelLabels[level]}</span>
      <span className="text-text-primary break-words flex-1">{message}</span>
    </div>
  );
}
