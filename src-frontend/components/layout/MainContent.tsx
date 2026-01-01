import { cn } from '@/lib/cn';

export interface MainContentProps {
  children: React.ReactNode;
  className?: string;
}

export function MainContent({ children, className }: MainContentProps) {
  return (
    <main className={cn('flex-1 ml-[260px] flex flex-col min-h-screen', className)}>
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
    <div className={cn('flex-1 p-6 overflow-y-auto', className)}>
      {children}
    </div>
  );
}
