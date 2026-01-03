import { cn } from '@/lib/cn';

export interface NavBadgeProps {
  count: number;
  className?: string;
}

export function NavBadge({ count, className }: NavBadgeProps) {
  return (
    <span
      className={cn(
        'bg-[var(--primary)]',
        'text-tiny font-semibold',
        'rounded-full min-w-[20px] text-center',
        className
      )}
      style={{
        padding: '2px 8px',
        color: '#FFFFFF',
      }}
    >
      {count > 99 ? '99+' : count}
    </span>
  );
}
