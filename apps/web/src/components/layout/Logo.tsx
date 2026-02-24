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
    <div className={cn('flex items-center gap-3', className)}>
      <img
        src="/app-icon.png"
        alt="SpiritStream"
        className={cn(
          'rounded-xl',
          'shadow-md',
          sizes[size]
        )}
      />
      {showText && (
        <span className="font-bold text-lg text-gradient">
          SpiritStream
        </span>
      )}
    </div>
  );
}
