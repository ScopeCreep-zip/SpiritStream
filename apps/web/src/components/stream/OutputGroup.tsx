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
  headerAction?: React.ReactNode; // Optional action (e.g. toggle switch) in header
  children: React.ReactNode;
  className?: string;
}

export function OutputGroup({
  name,
  info,
  status,
  defaultExpanded = false,
  headerAction,
  children,
  className,
}: OutputGroupProps) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(defaultExpanded);

  return (
    <div
      className={cn(
        'bg-bg-muted border border-border-default',
        'rounded-xl',
        'mb-4',
        className
      )}
    >
      <button
        className={cn(
          'w-full flex items-center justify-between cursor-pointer',
          'bg-transparent border-none text-left',
          'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default',
          'focus-visible:ring-inset rounded-xl',
          'py-4 px-5'
        )}
        onClick={() => setExpanded(!expanded)}
        aria-expanded={expanded}
      >
        <div className="flex items-center gap-3">
          <Layers className="w-[18px] h-[18px] text-primary" />
          <div>
            <div className="font-semibold text-text-primary">{name}</div>
            <div className="text-small text-text-secondary">{info}</div>
          </div>
        </div>
        <div className="flex items-center gap-3">
          {headerAction && <div onClick={(e) => e.stopPropagation()}>{headerAction}</div>}
          <StreamStatus
            status={status}
            label={status === 'offline' ? t('status.ready') : undefined}
          />
          <ChevronDown
            className={cn(
              'w-[18px] h-[18px] text-text-tertiary transition-transform duration-200',
              expanded && 'rotate-180'
            )}
          />
        </div>
      </button>
      {expanded && <div className="px-5 pb-5">{children}</div>}
    </div>
  );
}
