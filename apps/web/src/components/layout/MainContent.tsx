import { cn } from '@/lib/cn';

export interface MainContentProps {
  children: React.ReactNode;
  className?: string;
}

export function MainContent({ children, className }: MainContentProps) {
  return (
    <main
      className={cn('flex-1 flex flex-col min-h-screen', className)}
      style={{ marginLeft: '260px' }}
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
    <div className={cn('flex-1 overflow-y-auto', className)} style={{ padding: '24px' }}>
      {children}
    </div>
  );
}
