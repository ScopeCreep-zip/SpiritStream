import { cn } from '@/lib/cn';

export interface StatBoxProps {
  icon: React.ReactNode;
  label: string;
  value: string | number;
  change?: string;
  changeType?: 'positive' | 'neutral';
  className?: string;
}

export function StatBox({
  icon,
  label,
  value,
  change,
  changeType = 'neutral',
  className,
}: StatBoxProps) {
  return (
    <div
      className={cn(
        'bg-bg-surface border border-border-default',
        'rounded-xl',
        'p-5',
        className
      )}
    >
      <div className="flex items-center justify-between mb-2">
        <span className="text-small text-text-secondary">{label}</span>
        <span className="text-text-tertiary">{icon}</span>
      </div>
      <div className="text-2xl font-bold text-text-primary">{value}</div>
      {change && (
        <div
          className={cn(
            'text-xs mt-1',
            changeType === 'positive' ? 'text-success-text' : 'text-text-tertiary'
          )}
        >
          {change}
        </div>
      )}
    </div>
  );
}
