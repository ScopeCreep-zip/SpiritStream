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
        'bg-[var(--bg-sunken)] border border-[var(--border-default)] rounded-lg',
        "font-['JetBrains_Mono',monospace] text-xs overflow-y-auto",
        className
      )}
      style={{ maxHeight }}
    >
      {children}
    </div>
  );
}
