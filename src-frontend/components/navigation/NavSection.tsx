import { cn } from '@/lib/cn';

export interface NavSectionProps {
  title: string;
  children: React.ReactNode;
  className?: string;
}

export function NavSection({ title, children, className }: NavSectionProps) {
  return (
    <div className={cn('mb-6', className)}>
      <div
        className={cn(
          'text-[0.6875rem] font-semibold uppercase tracking-wider',
          'text-[var(--text-tertiary)] px-3 mb-2'
        )}
      >
        {title}
      </div>
      <div className="space-y-1">{children}</div>
    </div>
  );
}
