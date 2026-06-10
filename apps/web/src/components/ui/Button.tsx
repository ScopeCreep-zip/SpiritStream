import { forwardRef } from 'react';
import { cn } from '@/lib/cn';

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'accent' | 'ghost' | 'outline' | 'destructive';
  size?: 'sm' | 'md' | 'lg' | 'icon';
  loading?: boolean;
}

const VARIANT_CLASSES = {
  primary: 'bg-primary text-primary-foreground hover:bg-primary-hover active:bg-primary-active',
  secondary:
    'bg-secondary text-secondary-foreground hover:bg-secondary-hover active:bg-secondary-active',
  accent: 'bg-accent text-accent-foreground hover:bg-accent-hover active:bg-accent-active',
  ghost: 'bg-transparent text-text-secondary hover:bg-bg-hover hover:text-text-primary',
  outline: 'bg-transparent border-2 border-primary text-primary hover:bg-primary-subtle',
  destructive: 'bg-error text-error-foreground hover:bg-error-hover active:bg-error-hover',
} as const;

const SIZE_CLASSES = {
  sm: 'text-sm h-9 py-2 px-5',
  md: 'text-base h-11 py-2.5 px-6',
  lg: 'text-lg h-14 py-3 px-10',
  icon: 'w-10 h-10 p-0',
} as const;

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  (
    { className, variant = 'primary', size = 'md', loading, children, disabled, style, ...props },
    ref
  ) => {
    return (
      <button
        ref={ref}
        className={cn(
          'inline-flex items-center justify-center gap-2 font-semibold rounded-lg transition-all duration-150',
          'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default',
          'focus-visible:ring-offset-2 focus-visible:ring-offset-ring-offset',
          'disabled:opacity-50 disabled:cursor-not-allowed',
          'border-none cursor-pointer',
          VARIANT_CLASSES[variant],
          SIZE_CLASSES[size],
          className
        )}
        style={style}
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
