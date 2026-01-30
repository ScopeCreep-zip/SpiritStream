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
  /** Size variant - 'sm' for compact tables, 'default' for normal usage */
  size?: 'sm' | 'default';
}

// Size configurations
const sizes = {
  default: {
    track: 'w-11 h-6',
    thumb: 'w-[18px] h-[18px] left-[3px] bottom-[3px]',
    thumbTranslate: 'peer-checked:translate-x-5',
  },
  sm: {
    track: 'w-8 h-4',
    thumb: 'w-3 h-3 left-[2px] bottom-[2px]',
    thumbTranslate: 'peer-checked:translate-x-4',
  },
};

export function Toggle({
  checked = false,
  onChange,
  disabled,
  label,
  description,
  className,
  id,
  size = 'default',
}: ToggleProps) {
  const generatedId = useId();
  const toggleId = id || generatedId;
  const descriptionId = description ? `${toggleId}-description` : undefined;
  const sizeConfig = sizes[size];

  return (
    <label
      className={cn(
        'inline-flex items-center cursor-pointer',
        disabled && 'opacity-50 cursor-not-allowed',
        className
      )}
      style={{ gap: '12px' }}
    >
      <span className={cn('relative flex-shrink-0', sizeConfig.track)}>
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
            'bg-[var(--border-strong)]',
            'peer-checked:bg-[var(--primary)]',
            'peer-focus-visible:ring-[3px] peer-focus-visible:ring-[var(--ring-default)]',
            'peer-focus-visible:ring-offset-2 peer-focus-visible:ring-offset-[var(--ring-offset)]'
          )}
        />
        <span
          className={cn(
            'absolute',
            sizeConfig.thumb,
            'bg-white rounded-full shadow-[var(--shadow-sm)]',
            'transition-transform duration-200',
            sizeConfig.thumbTranslate
          )}
        />
      </span>
      {(label || description) && (
        <div className="flex flex-col">
          {label && <span className="text-sm font-medium text-[var(--text-primary)]">{label}</span>}
          {description && (
            <span id={descriptionId} className="text-xs text-[var(--text-tertiary)]">
              {description}
            </span>
          )}
        </div>
      )}
    </label>
  );
}
