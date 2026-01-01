import { cn } from '@/lib/cn';

export interface GridProps extends React.HTMLAttributes<HTMLDivElement> {
  cols?: 1 | 2 | 3 | 4;
  gap?: 'sm' | 'md' | 'lg';
}

export function Grid({ cols = 2, gap = 'md', className, ...props }: GridProps) {
  const colStyles = {
    1: 'grid-cols-1',
    2: 'grid-cols-2 max-lg:grid-cols-1',
    3: 'grid-cols-3 max-xl:grid-cols-2 max-md:grid-cols-1',
    4: 'grid-cols-4 max-xl:grid-cols-2 max-md:grid-cols-1',
  };

  const gapStyles = {
    sm: 'gap-3',
    md: 'gap-4',
    lg: 'gap-6',
  };

  return (
    <div
      className={cn('grid', colStyles[cols], gapStyles[gap], className)}
      {...props}
    />
  );
}
