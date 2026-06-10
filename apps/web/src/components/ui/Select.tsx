import { forwardRef, useId } from 'react';
import { ChevronDown } from 'lucide-react';
import { cn } from '@/lib/cn';
import { FormGroup, FormLabel, FormHelper, FormError } from './Form';

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
    const generatedId = useId();
    const selectId = id || generatedId;
    const helperId = helper && !error ? `${selectId}-helper` : undefined;
    const errorId = error ? `${selectId}-error` : undefined;
    const describedBy = errorId || helperId;

    return (
      <FormGroup>
        {label && (
          <FormLabel htmlFor={selectId}>{label}</FormLabel>
        )}
        <div className="relative">
          <select
            ref={ref}
            id={selectId}
            aria-invalid={error ? true : undefined}
            aria-describedby={describedBy}
            className={cn(
              'w-full text-sm rounded-lg transition-all duration-150',
              'bg-bg-sunken text-text-primary',
              'border-2 border-border-strong',
              'hover:border-border-stronger',
              'focus:outline-none focus:border-border-interactive',
              'focus:ring-[3px] focus:ring-primary-muted',
              'disabled:opacity-50 disabled:cursor-not-allowed disabled:bg-bg-muted',
              'appearance-none cursor-pointer',
              error && 'border-error-border',
              'py-2.5 pe-10 ps-3.5',
              className
            )}
            {...props}
          >
            {options.map((option) => (
              <option key={option.value} value={option.value} disabled={option.disabled}>
                {option.label}
              </option>
            ))}
          </select>
          <ChevronDown
            className="absolute end-3 top-1/2 -translate-y-1/2 w-4 h-4 text-text-tertiary pointer-events-none"
            aria-hidden="true"
          />
        </div>
        {helper && !error && <FormHelper id={helperId}>{helper}</FormHelper>}
        {error && <FormError id={errorId}>{error}</FormError>}
      </FormGroup>
    );
  }
);

Select.displayName = 'Select';
