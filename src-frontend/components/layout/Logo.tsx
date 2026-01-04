import { cn } from '@/lib/cn';

export interface LogoProps {
  size?: 'sm' | 'md' | 'lg';
  showText?: boolean;
  className?: string;
}

export function Logo({ size = 'md', showText = true, className }: LogoProps) {
  const sizes = {
    sm: 'w-8 h-8',
    md: 'w-10 h-10',
    lg: 'w-12 h-12',
  };

  return (
    <div className={cn('flex items-center', className)} style={{ gap: '12px' }}>
      <img
        src="/app-icon.png"
        alt="SpiritStream"
        className={cn(
          'rounded-xl',
          'shadow-[var(--shadow-md)]',
          sizes[size]
        )}
      />
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
