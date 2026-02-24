import { cn } from '@/lib/cn';

export interface SidebarProps {
  children: React.ReactNode;
  className?: string;
}

export function Sidebar({ children, className }: SidebarProps) {
  return (
    <aside
      className={cn(
        'w-[260px] bg-bg-surface border-r border-border-default',
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
      className={cn('border-b border-border-muted', 'flex items-center gap-3 py-5 px-4', className)}
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
    <nav className={cn('flex-1 overflow-y-auto py-4 px-3', className)}>
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
    <div
      className={cn('border-t border-border-muted p-4', className)}
    >
      {children}
    </div>
  );
}
