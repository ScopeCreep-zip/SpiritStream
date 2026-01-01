import { cn } from '@/lib/cn';

export interface StatsRowProps {
  children: React.ReactNode;
  className?: string;
}

export function StatsRow({ children, className }: StatsRowProps) {
  return (
    <div
      className={cn(
        'grid grid-cols-4 gap-4 mb-6',
        'max-xl:grid-cols-2 max-md:grid-cols-1',
        className
      )}
    >
      {children}
    </div>
  );
}
