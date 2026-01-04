import { forwardRef, useId } from 'react';
import { cn } from '@/lib/cn';

export interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
  helper?: string;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ className, label, error, helper, id, type = 'text', ...props }, ref) => {
    const generatedId = useId();
    const inputId = id || generatedId;
    const helperId = helper && !error ? `${inputId}-helper` : undefined;
    const errorId = error ? `${inputId}-error` : undefined;
    const describedBy = errorId || helperId;

    return (
      <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
        {label && (
          <label htmlFor={inputId} className="block text-sm font-medium text-[var(--text-primary)]">
            {label}
          </label>
        )}
        <input
          ref={ref}
          id={inputId}
          type={type}
          aria-invalid={error ? true : undefined}
          aria-describedby={describedBy}
          className={cn(
            'w-full text-sm rounded-lg transition-all duration-150',
            'bg-[var(--bg-sunken)] text-[var(--text-primary)]',
            'border-2 border-[var(--border-strong)]',
            'placeholder:text-[var(--text-muted)]',
            'hover:border-[var(--border-stronger)]',
            'focus:outline-none focus:border-[var(--border-interactive)]',
            'focus:ring-[3px] focus:ring-[var(--primary-muted)]',
            'disabled:opacity-50 disabled:cursor-not-allowed disabled:bg-[var(--bg-muted)]',
            error && 'border-[var(--error-border)] focus:ring-[var(--error-subtle)]',
            className
          )}
          style={{ padding: '10px 14px' }}
          {...props}
        />
        {helper && !error && (
          <p id={helperId} className="text-xs text-[var(--text-tertiary)]">
            {helper}
          </p>
        )}
        {error && (
          <p id={errorId} className="text-xs text-[var(--error-text)]" role="alert">
            {error}
          </p>
        )}
      </div>
    );
  }
);

Input.displayName = 'Input';
