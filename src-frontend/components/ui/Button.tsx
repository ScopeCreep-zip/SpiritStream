import { forwardRef } from 'react';
import { cn } from '@/lib/cn';

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'accent' | 'ghost' | 'outline' | 'destructive';
  size?: 'sm' | 'md' | 'lg' | 'icon';
  loading?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  (
    { className, variant = 'primary', size = 'md', loading, children, disabled, style, ...props },
    ref
  ) => {
    const variants = {
      primary:
        'bg-[var(--primary)] text-white hover:bg-[var(--primary-hover)] active:bg-[var(--primary-active)]',
      secondary: 'bg-[var(--secondary)] text-white hover:opacity-90',
      accent: 'bg-[var(--accent)] text-white hover:opacity-90',
      ghost:
        'bg-transparent text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]',
      outline:
        'bg-transparent border-2 border-[var(--primary)] text-[var(--primary)] hover:bg-[var(--primary-subtle)]',
      destructive: 'bg-[var(--error)] text-white hover:opacity-90',
    };

    const sizeClasses = {
      sm: 'text-sm',
      md: 'text-base',
      lg: 'text-lg',
      icon: '',
    };

    const sizeStyles: Record<string, React.CSSProperties> = {
      sm: { height: '36px', padding: '8px 20px' },
      md: { height: '44px', padding: '10px 24px' },
      lg: { height: '56px', padding: '12px 40px' },
      icon: { width: '40px', height: '40px', padding: '0' },
    };

    return (
      <button
        ref={ref}
        className={cn(
          'inline-flex items-center justify-center font-semibold rounded-lg transition-all duration-150',
          'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-[var(--ring-default)]',
          'focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--ring-offset)]',
          'disabled:opacity-50 disabled:cursor-not-allowed',
          'border-none cursor-pointer',
          variants[variant],
          sizeClasses[size],
          className
        )}
        style={{ gap: '8px', ...sizeStyles[size], ...style }}
        disabled={disabled || loading}
        {...props}
      >
        {loading && <Spinner className="w-4 h-4" />}
        {children}
      </button>
    );
  }
);

Button.displayName = 'Button';

function Spinner({ className }: { className?: string }) {
  return (
    <svg
      className={cn('animate-spin', className)}
      xmlns="http://www.w3.org/2000/svg"
      fill="none"
      viewBox="0 0 24 24"
    >
      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
      <path
        className="opacity-75"
        fill="currentColor"
        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
      />
    </svg>
  );
}
