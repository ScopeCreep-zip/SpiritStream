import { cn } from '@/lib/cn';

export interface HeaderProps {
  title: string;
  description?: string;
  children?: React.ReactNode;
  className?: string;
}

export function Header({ title, description, children, className }: HeaderProps) {
  return (
    <header
      className={cn(
        'bg-[var(--bg-surface)] border-b border-[var(--border-default)]',
        'flex items-center justify-between',
        'sticky top-0 z-50',
        className
      )}
      style={{ padding: '16px 24px' }}
    >
      <div className="flex flex-col">
        <h1 className="text-xl font-semibold text-[var(--text-primary)]">
          {title}
        </h1>
        {description && (
          <p className="text-sm text-[var(--text-secondary)] mt-0.5">
            {description}
          </p>
        )}
      </div>
      {children && (
        <div className="flex items-center gap-3">{children}</div>
      )}
    </header>
  );
}
