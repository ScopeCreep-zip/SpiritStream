import { forwardRef } from 'react';
import { cn } from '@/lib/cn';

export interface SelectOption {
  value: string;
  label: string;
  disabled?: boolean;
}

export interface SelectProps extends React.SelectHTMLAttributes<HTMLSelectElement> {
  label?: string;
  error?: string;
  helper?: string;
  options: SelectOption[];
}

export const Select = forwardRef<HTMLSelectElement, SelectProps>(
  ({ className, label, error, helper, options, id, ...props }, ref) => {
    const selectId = id || label?.toLowerCase().replace(/\s/g, '-');

    return (
      <div className="space-y-1.5">
        {label && (
          <label
            htmlFor={selectId}
            className="block text-sm font-medium text-[var(--text-primary)]"
          >
            {label}
          </label>
        )}
        <select
          ref={ref}
          id={selectId}
          className={cn(
            'w-full px-3.5 py-2.5 text-sm rounded-lg transition-all duration-150',
            'bg-[var(--bg-sunken)] text-[var(--text-primary)]',
            'border-2 border-[var(--border-strong)]',
            'hover:border-[var(--border-stronger)]',
            'focus:outline-none focus:border-[var(--border-interactive)]',
            'focus:shadow-[0_0_0_3px_var(--primary-muted)]',
            'disabled:opacity-50 disabled:cursor-not-allowed disabled:bg-[var(--bg-muted)]',
            'appearance-none cursor-pointer',
            'bg-[length:16px] bg-no-repeat bg-[right_0.75rem_center]',
            "bg-[url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='16' height='16' viewBox='0 0 24 24' fill='none' stroke='%23756A8A' stroke-width='2'%3E%3Cpolyline points='6 9 12 15 18 9'/%3E%3C/svg%3E\")]",
            'pr-10',
            error && 'border-[var(--error-border)]',
            className
          )}
          {...props}
        >
          {options.map((option) => (
            <option
              key={option.value}
              value={option.value}
              disabled={option.disabled}
            >
              {option.label}
            </option>
          ))}
        </select>
        {helper && !error && (
          <p className="text-xs text-[var(--text-tertiary)]">{helper}</p>
        )}
        {error && (
          <p className="text-xs text-[var(--error-text)]">{error}</p>
        )}
      </div>
    );
  }
);

Select.displayName = 'Select';
