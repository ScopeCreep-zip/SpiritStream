import { useId } from 'react';
import { cn } from '@/lib/cn';

export interface ToggleProps {
  checked?: boolean;
  onChange?: (checked: boolean) => void;
  disabled?: boolean;
  label?: string;
  description?: string;
  className?: string;
  id?: string;
}

export function Toggle({
  checked = false,
  onChange,
  disabled,
  label,
  description,
  className,
  id,
}: ToggleProps) {
  const generatedId = useId();
  const toggleId = id || generatedId;
  const descriptionId = description ? `${toggleId}-description` : undefined;

  return (
    <label
      className={cn(
        'inline-flex items-center gap-3 cursor-pointer',
        disabled && 'opacity-50 cursor-not-allowed',
        className
      )}
    >
      <span className="relative w-11 h-6 flex-shrink-0">
        <input
          type="checkbox"
          id={toggleId}
          checked={checked}
          onChange={(e) => onChange?.(e.target.checked)}
          disabled={disabled}
          className="sr-only peer"
          role="switch"
          aria-checked={checked}
          aria-describedby={descriptionId}
        />
        <span
          className={cn(
            'absolute inset-0 rounded-full transition-colors duration-200',
            'bg-border-strong',
            'peer-checked:bg-primary',
            'peer-focus-visible:ring-[3px] peer-focus-visible:ring-ring-default',
            'peer-focus-visible:ring-offset-2 peer-focus-visible:ring-offset-ring-offset'
          )}
        />
        <span
          className={cn(
            'absolute w-[18px] h-[18px] start-[3px] bottom-[3px]',
            'bg-white rounded-full shadow-sm',
            'transition-transform duration-200',
            'peer-checked:translate-x-5'
          )}
        />
      </span>
      {(label || description) && (
        <div className="flex flex-col">
          {label && <span className="text-sm font-medium text-text-primary">{label}</span>}
          {description && (
            <span id={descriptionId} className="text-xs text-text-tertiary">
              {description}
            </span>
          )}
        </div>
      )}
    </label>
  );
}
