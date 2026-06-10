import { cn } from '@/lib/cn';

export interface LogConsoleProps {
  children: React.ReactNode;
  maxHeight?: string;
  className?: string;
}

export function LogConsole({ children, maxHeight = '300px', className }: LogConsoleProps) {
  return (
    <div
      className={cn(
        'bg-bg-sunken border border-border-default rounded-lg',
        "font-['JetBrains_Mono',monospace] text-xs overflow-y-auto",
        'max-h-[var(--log-max-height)]',
        className
      )}
      // Caller-supplied dimension — inject as a CSS var (Modal.tsx pattern).
      style={{ '--log-max-height': maxHeight } as React.CSSProperties}
    >
      {children}
    </div>
  );
}
