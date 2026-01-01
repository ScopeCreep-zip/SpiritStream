import { cn } from '@/lib/cn';

export interface NavBadgeProps {
  count: number;
  className?: string;
}

export function NavBadge({ count, className }: NavBadgeProps) {
  return (
    <span
      className={cn(
        'bg-[var(--primary)] text-white',
        'text-[0.6875rem] font-semibold',
        'px-2 py-0.5 rounded-full min-w-[20px] text-center',
        className
      )}
    >
      {count > 99 ? '99+' : count}
    </span>
  );
}
