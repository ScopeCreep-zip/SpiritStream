import { cn } from '@/lib/cn';

export interface AppShellProps {
  children: React.ReactNode;
  className?: string;
}

export function AppShell({ children, className }: AppShellProps) {
  return (
    <div className={cn('flex min-h-screen bg-[var(--bg-base)]', className)}>
      {children}
    </div>
  );
}
