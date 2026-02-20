import { forwardRef, memo } from 'react';
import { cn } from '@/lib/cn';

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'accent' | 'ghost' | 'outline' | 'destructive';
  size?: 'sm' | 'md' | 'lg' | 'icon';
  loading?: boolean;
}

const VARIANTS: Record<string, string> = {
  primary: 'btn-primary',
  secondary: 'btn-secondary',
  accent: 'btn-accent',
  ghost: 'btn-ghost',
  outline: 'btn-outline',
  destructive: 'btn-destructive',
};

const SIZE_CLASSES: Record<string, string> = {
  sm: 'h-9 px-5 py-2 text-sm gap-2',
  md: 'h-11 px-6 py-2.5 text-base gap-2',
  lg: 'h-14 px-10 py-3 text-lg gap-2',
  icon: 'w-10 h-10 p-0',
};

export const Button = memo(forwardRef<HTMLButtonElement, ButtonProps>(
  (
    { className, variant = 'primary', size = 'md', loading, children, disabled, ...props },
    ref
  ) => {
    return (
      <button
        ref={ref}
        className={cn(
          'inline-flex items-center justify-center font-semibold rounded-lg transition-colors duration-150',
          'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-[var(--ring-default)]',
          'focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--ring-offset)]',
          'disabled:opacity-50 disabled:cursor-not-allowed',
          'border-none cursor-pointer',
          VARIANTS[variant],
          SIZE_CLASSES[size],
          className
        )}
        disabled={disabled || loading}
        {...props}
      >
        {loading && <Spinner className="w-4 h-4" />}
        {children}
      </button>
    );
  }
));

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
