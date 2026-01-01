import { useState } from 'react';
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
  const [expanded, setExpanded] = useState(defaultExpanded);

  return (
    <div
      className={cn(
        'bg-[var(--bg-muted)] border border-[var(--border-default)]',
        'rounded-xl mb-4',
        className
      )}
    >
      <button
        className={cn(
          'w-full p-4 px-5 flex items-center justify-between cursor-pointer',
          'bg-transparent border-none text-left',
          'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-[var(--ring-default)]',
          'focus-visible:ring-inset rounded-xl'
        )}
        onClick={() => setExpanded(!expanded)}
        aria-expanded={expanded}
      >
        <div className="flex items-center gap-3">
          <Layers className="w-[18px] h-[18px] text-[var(--primary)]" />
          <div>
            <div className="font-semibold text-[var(--text-primary)]">{name}</div>
            <div className="text-[0.8125rem] text-[var(--text-secondary)]">
              {info}
            </div>
          </div>
        </div>
        <div className="flex items-center gap-3">
          <StreamStatus
            status={status}
            label={status === 'offline' ? 'Ready' : undefined}
          />
          <ChevronDown
            className={cn(
              'w-[18px] h-[18px] text-[var(--text-tertiary)] transition-transform duration-200',
              expanded && 'rotate-180'
            )}
          />
        </div>
      </button>
      {expanded && <div className="px-5 pb-5">{children}</div>}
    </div>
  );
}
