import React, { useId } from 'react';

/**
 * `FormField` — accessible wrapper that pairs a label, an
 * arbitrary control, and helper/error text with proper ARIA linkage
 * (`aria-describedby` → helper, `aria-invalid` → error). Use this for
 * custom controls (textareas, custom widgets, control composites) where
 * the existing `Input` / `Select` primitives don't apply, since those
 * already own their own label-and-error rendering.
 *
 * The `children` render prop receives the id/aria props the control must
 * spread onto itself so screen readers announce label and helper/error
 * together.
 */
export interface FormFieldProps {
  label: string;
  helper?: string;
  error?: string;
  required?: boolean;
  children: (controlProps: {
    id: string;
    'aria-describedby'?: string;
    'aria-invalid'?: boolean;
  }) => React.ReactNode;
}

export function FormField({
  label,
  helper,
  error,
  required,
  children,
}: FormFieldProps): React.ReactElement {
  const fieldId = useId();
  const helperId = helper ? `${fieldId}-helper` : undefined;
  const errorId = error ? `${fieldId}-error` : undefined;
  const describedBy = [errorId, helperId].filter(Boolean).join(' ') || undefined;

  return (
    <div className="flex flex-col gap-1">
      <label htmlFor={fieldId} className="text-sm font-medium text-text-primary">
        {label}
        {required && <span aria-hidden="true" className="text-error-text"> *</span>}
      </label>
      {children({
        id: fieldId,
        'aria-describedby': describedBy,
        'aria-invalid': error ? true : undefined,
      })}
      {error && (
        <p id={errorId} role="alert" className="text-xs text-error-text">
          {error}
        </p>
      )}
      {!error && helper && (
        <p id={helperId} className="text-xs text-text-tertiary">
          {helper}
        </p>
      )}
    </div>
  );
}
