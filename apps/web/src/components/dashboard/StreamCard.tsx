import { memo } from 'react';
import { cn } from '@/lib/cn';
import { StreamStatus } from '@/components/ui/StreamStatus';
import { PlatformIcon } from '@/components/stream/PlatformIcon';
import type { Platform } from '@spiritstream/types';
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

export const StreamCard = memo(function StreamCard({ platform, name, status, stats, onClick, className }: StreamCardProps) {
  return (
    <div
      onClick={onClick}
      className={cn(
        'bg-bg-surface border border-border-default',
        'rounded-xl transition-all duration-150',
        'hover:border-border-interactive hover:shadow-md',
        onClick && 'cursor-pointer',
        'p-4',
        className
      )}
    >
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <PlatformIcon platform={platform} />
          <span className="font-semibold text-sm text-text-primary">{name}</span>
        </div>
        <StreamStatus status={status} />
      </div>
      {stats && stats.length > 0 && (
        <div className="grid grid-cols-3 gap-3 pt-3 border-t border-border-muted">
          {stats.map((stat, index) => (
            <div key={index} className="text-center">
              <div className="text-sm font-semibold text-text-primary">{stat.value}</div>
              <div className="text-tiny uppercase text-text-tertiary">{stat.label}</div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
});
