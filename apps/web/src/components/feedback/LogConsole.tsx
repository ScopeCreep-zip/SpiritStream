import { cn } from '@/lib/cn';

const MAX_HEIGHT_CLASSES = {
  sm: 'max-h-[200px]',
  md: 'max-h-[300px]',
  lg: 'max-h-[500px]',
} as const;

export interface LogConsoleProps {
  children: React.ReactNode;
  maxHeight?: 'sm' | 'md' | 'lg';
  className?: string;
}

export function LogConsole({ children, maxHeight = 'md', className }: LogConsoleProps) {
  return (
    <div
      className={cn(
        'bg-bg-sunken border border-border-default rounded-lg',
        "font-['JetBrains_Mono',monospace] text-xs overflow-y-auto",
        MAX_HEIGHT_CLASSES[maxHeight],
        className
      )}
    >
      {children}
    </div>
  );
}
