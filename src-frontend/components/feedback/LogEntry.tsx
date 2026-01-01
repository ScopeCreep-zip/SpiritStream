import { cn } from '@/lib/cn';
import type { LogLevel } from '@/types/stream';

export interface LogEntryProps {
  time: string;
  level: LogLevel;
  message: string;
}

const levelStyles: Record<LogLevel, string> = {
  info: 'text-[var(--primary)]',
  warn: 'text-[var(--warning-text)]',
  error: 'text-[var(--error-text)]',
  debug: 'text-[var(--text-tertiary)]',
};

const levelLabels: Record<LogLevel, string> = {
  info: 'INFO',
  warn: 'WARN',
  error: 'ERROR',
  debug: 'DEBUG',
};

export function LogEntry({ time, level, message }: LogEntryProps) {
  return (
    <div className="px-3 py-1.5 flex gap-3 border-b border-[var(--border-muted)] last:border-b-0">
      <span className="text-[var(--text-muted)] whitespace-nowrap">{time}</span>
      <span className={cn('font-semibold w-12', levelStyles[level])}>
        {levelLabels[level]}
      </span>
      <span className="text-[var(--text-primary)] break-words flex-1">
        {message}
      </span>
    </div>
  );
}
