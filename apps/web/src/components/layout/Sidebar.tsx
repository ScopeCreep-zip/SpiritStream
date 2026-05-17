import { cn } from '@/lib/cn';

export interface SidebarProps {
  children: React.ReactNode;
  className?: string;
}

export function Sidebar({ children, className }: SidebarProps) {
  return (
    <aside
      className={cn(
        // `border-e` and `start-0` (logical) replace
        // `border-r` and `left-0` so the sidebar mirrors to the right
        // edge under `dir="rtl"`. `z-[var(--z-sidebar)]` reaches the
        // centralised z-ladder.
        'w-[260px] bg-bg-surface border-e border-border-default',
        'flex flex-col fixed top-0 start-0 bottom-0 z-[var(--z-sidebar)]',
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
