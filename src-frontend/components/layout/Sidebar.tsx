import { cn } from '@/lib/cn';

export interface SidebarProps {
  children: React.ReactNode;
  className?: string;
}

export function Sidebar({ children, className }: SidebarProps) {
  return (
    <aside
      className={cn(
        'w-sidebar bg-[var(--bg-surface)] border-r border-[var(--border-default)]',
        'flex flex-col fixed top-0 left-0 bottom-0 z-[100]',
        className
      )}
    >
      {children}
    </aside>
  );
}

export interface SidebarHeaderProps {
  children: React.ReactNode;
  className?: string;
}

export function SidebarHeader({ children, className }: SidebarHeaderProps) {
  return (
    <div
      className={cn(
        'border-b border-[var(--border-muted)]',
        'flex items-center gap-3',
        className
      )}
      style={{ padding: '20px 16px' }}
    >
      {children}
    </div>
  );
}

export interface SidebarNavProps {
  children: React.ReactNode;
  className?: string;
}

export function SidebarNav({ children, className }: SidebarNavProps) {
  return (
    <nav className={cn('flex-1 overflow-y-auto', className)} style={{ padding: '16px 12px' }}>
      {children}
    </nav>
  );
}

export interface SidebarFooterProps {
  children: React.ReactNode;
  className?: string;
}

export function SidebarFooter({ children, className }: SidebarFooterProps) {
  return (
    <div className={cn('border-t border-[var(--border-muted)]', className)} style={{ padding: '16px' }}>
      {children}
    </div>
  );
}
