import { forwardRef } from 'react';
import { ChevronDown } from 'lucide-react';
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
      <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
        {label && (
          <label
            htmlFor={selectId}
            className="block text-sm font-medium text-[var(--text-primary)]"
          >
            {label}
          </label>
        )}
        <div className="relative">
          <select
            ref={ref}
            id={selectId}
            className={cn(
              'w-full text-sm rounded-lg transition-all duration-150',
              'bg-[var(--bg-sunken)] text-[var(--text-primary)]',
              'border-2 border-[var(--border-strong)]',
              'hover:border-[var(--border-stronger)]',
              'focus:outline-none focus:border-[var(--border-interactive)]',
              'focus:ring-[3px] focus:ring-[var(--primary-muted)]',
              'disabled:opacity-50 disabled:cursor-not-allowed disabled:bg-[var(--bg-muted)]',
              'appearance-none cursor-pointer',
              error && 'border-[var(--error-border)]',
              className
            )}
            style={{ padding: '10px 40px 10px 14px' }}
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
          <ChevronDown
            className="absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-[var(--text-tertiary)] pointer-events-none"
          />
        </div>
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
