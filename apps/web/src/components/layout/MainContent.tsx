import { cn } from '@/lib/cn';

export interface MainContentProps {
  children: React.ReactNode;
  className?: string;
}

export function MainContent({ children, className }: MainContentProps) {
  return (
    <main
      id="main-content"
      // `ms-[260px]` is the logical-property form of
      // `ml-[260px]`. Under RTL the sidebar lives on the right and
      // the main column needs margin-inline-start (which becomes
      // margin-right). Using the logical property means the layout
      // mirrors correctly without per-direction CSS branches.
      className={cn('flex-1 flex flex-col min-h-screen ms-[260px]', className)}
      tabIndex={-1}
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
    <div className={cn('flex-1 overflow-y-auto p-6', className)}>
      {children}
    </div>
  );
}
