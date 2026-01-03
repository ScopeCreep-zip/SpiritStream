import { cn } from '@/lib/cn';

export interface LogoProps {
  size?: 'sm' | 'md' | 'lg';
  showText?: boolean;
  className?: string;
}

export function Logo({ size = 'md', showText = true, className }: LogoProps) {
  const sizes = {
    sm: 'w-8 h-8 text-base',
    md: 'w-10 h-10 text-xl',
    lg: 'w-12 h-12 text-2xl',
  };

  return (
    <div className={cn('flex items-center', className)} style={{ gap: '12px' }}>
      <div
        className={cn(
          'rounded-xl',
          'flex items-center justify-center',
          'font-bold shadow-[var(--shadow-md)]',
          sizes[size]
        )}
        style={{
          background: 'var(--gradient-brand)',
          color: 'var(--primary-foreground)',
        }}
      >
        S
      </div>
      {showText && (
        <span
          className="font-bold text-lg bg-clip-text"
          style={{
            background: 'var(--gradient-brand)',
            WebkitBackgroundClip: 'text',
            WebkitTextFillColor: 'transparent',
          }}
        >
          SpiritStream
        </span>
      )}
    </div>
  );
}
