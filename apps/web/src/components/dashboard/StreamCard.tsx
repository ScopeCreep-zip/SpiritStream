import { cn } from '@/lib/cn';
import { StreamStatus } from '@/components/ui/StreamStatus';
import { PlatformIcon } from '@/components/stream/PlatformIcon';
import type { Platform } from '@/types/profile';
import type { StreamStatusType } from '@/types/stream';

export interface StreamStat {
  label: string;
  value: string | number;
}

export interface StreamCardProps {
  platform: Platform;
  name: string;
  status: StreamStatusType;
  stats?: StreamStat[];
  onClick?: () => void;
  className?: string;
}

export function StreamCard({ platform, name, status, stats, onClick, className }: StreamCardProps) {
  return (
    <div
      onClick={onClick}
      className={cn(
        'bg-[var(--bg-surface)] border border-[var(--border-default)]',
        'rounded-xl transition-all duration-150',
        'hover:border-[var(--border-interactive)] hover:shadow-[var(--shadow-md)]',
        onClick && 'cursor-pointer',
        className
      )}
      style={{ padding: '16px' }}
    >
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <PlatformIcon platform={platform} />
          <span className="font-semibold text-sm text-[var(--text-primary)]">{name}</span>
        </div>
        <StreamStatus status={status} />
      </div>
      {stats && stats.length > 0 && (
        <div className="grid grid-cols-3 gap-3 pt-3 border-t border-[var(--border-muted)]">
          {stats.map((stat, index) => (
            <div key={index} className="text-center">
              <div className="text-sm font-semibold text-[var(--text-primary)]">{stat.value}</div>
              <div className="text-tiny uppercase text-[var(--text-tertiary)]">{stat.label}</div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
