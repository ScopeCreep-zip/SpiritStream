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
        'bg-[var(--bg-surface)] border border-[var(--border-default)]',
        'rounded-xl',
        className
      )}
      style={{ padding: '20px' }}
    >
      <div className="flex items-center justify-between mb-2">
        <span className="text-small text-[var(--text-secondary)]">{label}</span>
        <span className="text-[var(--text-tertiary)]">{icon}</span>
      </div>
      <div className="text-2xl font-bold text-[var(--text-primary)]">{value}</div>
      {change && (
        <div
          className={cn(
            'text-xs mt-1',
            changeType === 'positive' ? 'text-[var(--success-text)]' : 'text-[var(--text-tertiary)]'
          )}
        >
          {change}
        </div>
      )}
    </div>
  );
}
