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
          'text-tiny font-semibold uppercase tracking-wider',
          'text-text-tertiary',
          'px-3 mb-2'
        )}
      >
        {title}
      </div>
      <div className="flex flex-col gap-1">{children}</div>
    </div>
  );
}
