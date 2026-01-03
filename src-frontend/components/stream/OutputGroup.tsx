import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronDown, Layers } from 'lucide-react';
import { cn } from '@/lib/cn';
import { StreamStatus } from '@/components/ui/StreamStatus';
import type { StreamStatusType } from '@/types/stream';

export interface OutputGroupProps {
  name: string;
  info: string;
  status: StreamStatusType;
  defaultExpanded?: boolean;
  children: React.ReactNode;
  className?: string;
}

export function OutputGroup({
  name,
  info,
  status,
  defaultExpanded = false,
  children,
  className,
}: OutputGroupProps) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(defaultExpanded);

  return (
    <div
      className={cn(
        'bg-[var(--bg-muted)] border border-[var(--border-default)]',
        'rounded-xl',
        className
      )}
      style={{ marginBottom: '16px' }}
    >
      <button
        className={cn(
          'w-full flex items-center justify-between cursor-pointer',
          'bg-transparent border-none text-left',
          'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-[var(--ring-default)]',
          'focus-visible:ring-inset rounded-xl'
        )}
        style={{ padding: '16px 20px' }}
        onClick={() => setExpanded(!expanded)}
        aria-expanded={expanded}
      >
        <div className="flex items-center gap-3">
          <Layers className="w-[18px] h-[18px] text-[var(--primary)]" />
          <div>
            <div className="font-semibold text-[var(--text-primary)]">{name}</div>
            <div className="text-small text-[var(--text-secondary)]">
              {info}
            </div>
          </div>
        </div>
        <div className="flex items-center gap-3">
          <StreamStatus
            status={status}
            label={status === 'offline' ? t('status.ready') : undefined}
          />
          <ChevronDown
            className={cn(
              'w-[18px] h-[18px] text-[var(--text-tertiary)] transition-transform duration-200',
              expanded && 'rotate-180'
            )}
          />
        </div>
      </button>
      {expanded && <div style={{ padding: '0 20px 20px 20px' }}>{children}</div>}
    </div>
  );
}
