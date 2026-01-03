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
          'bg-gradient-to-br from-violet-600 via-fuchsia-600 to-pink-600 rounded-xl',
          'flex items-center justify-center',
          'text-white font-bold shadow-[var(--shadow-md)]',
          sizes[size]
        )}
      >
        S
      </div>
      {showText && (
        <span
          className="font-bold text-lg bg-gradient-to-r from-violet-600 via-fuchsia-600 to-pink-600 bg-clip-text"
          style={{ WebkitTextFillColor: 'transparent' }}
        >
          SpiritStream
        </span>
      )}
    </div>
  );
}
