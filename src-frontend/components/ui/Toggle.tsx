import { cn } from '@/lib/cn';

export interface ToggleProps {
  checked?: boolean;
  onChange?: (checked: boolean) => void;
  disabled?: boolean;
  label?: string;
  description?: string;
  className?: string;
}

export function Toggle({
  checked = false,
  onChange,
  disabled,
  label,
  description,
  className,
}: ToggleProps) {
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
          checked={checked}
          onChange={(e) => onChange?.(e.target.checked)}
          disabled={disabled}
          className="sr-only peer"
        />
        <span
          className={cn(
            'absolute inset-0 rounded-full transition-colors duration-200',
            'bg-[var(--border-strong)]',
            'peer-checked:bg-[var(--primary)]',
            'peer-focus-visible:ring-[3px] peer-focus-visible:ring-[var(--ring-default)]',
            'peer-focus-visible:ring-offset-2 peer-focus-visible:ring-offset-[var(--ring-offset)]'
          )}
        />
        <span
          className={cn(
            'absolute w-[18px] h-[18px] left-[3px] bottom-[3px]',
            'bg-white rounded-full shadow-[var(--shadow-sm)]',
            'transition-transform duration-200',
            'peer-checked:translate-x-5'
          )}
        />
      </span>
      {(label || description) && (
        <div className="flex flex-col">
          {label && (
            <span className="text-sm font-medium text-[var(--text-primary)]">
              {label}
            </span>
          )}
          {description && (
            <span className="text-xs text-[var(--text-tertiary)]">
              {description}
            </span>
          )}
        </div>
      )}
    </label>
  );
}
