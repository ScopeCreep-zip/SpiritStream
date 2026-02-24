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
        'bg-bg-surface border-b border-border-default',
        'flex items-center justify-between',
        'sticky top-0 z-50',
        'py-4 px-6',
        className
      )}
    >
      <div className="flex flex-col">
        <h1 className="text-xl font-semibold text-text-primary">{title}</h1>
        {description && (
          <p className="text-sm text-text-secondary mt-0.5">{description}</p>
        )}
      </div>
      {children && <div className="flex items-center gap-3">{children}</div>}
    </header>
  );
}
