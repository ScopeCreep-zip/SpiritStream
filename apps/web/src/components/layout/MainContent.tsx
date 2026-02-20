import { cn } from '@/lib/cn';

export interface MainContentProps {
  children: React.ReactNode;
  className?: string;
}

export function MainContent({ children, className }: MainContentProps) {
  return (
    <main
      className={cn('flex-1 flex flex-col h-screen ml-sidebar', className)}
    >
      {children}
    </main>
  );
}

export interface ContentAreaProps {
  children: React.ReactNode;
  className?: string;
}

export function ContentArea({ children, className }: ContentAreaProps) {
  return (
    <div className={cn('flex-1 overflow-y-auto', 'p-6', className)}>
      {children}
    </div>
  );
}
