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
      <div className="flex flex-col gap-1.5">
        {label && (
          <label htmlFor={inputId} className="block text-sm font-medium text-text-primary">
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
            'bg-bg-sunken text-text-primary',
            'border-2 border-border-strong',
            'placeholder:text-text-muted',
            'hover:border-border-stronger',
            'focus:outline-none focus:border-border-interactive',
            'focus:ring-[3px] focus:ring-primary-muted',
            'disabled:opacity-50 disabled:cursor-not-allowed disabled:bg-bg-muted',
            error && 'border-error-border focus:ring-error-subtle',
            'py-2.5 px-3.5',
            className
          )}
          {...props}
        />
        {helper && !error && (
          <p id={helperId} className="text-xs text-text-tertiary">
            {helper}
          </p>
        )}
        {error && (
          <p id={errorId} className="text-xs text-error-text" role="alert">
            {error}
          </p>
        )}
      </div>
    );
  }
);

Input.displayName = 'Input';
