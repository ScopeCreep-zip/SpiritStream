import React, { useId } from 'react';

/**
 * `FormField` — accessible wrapper that pairs a label, an arbitrary
 * control, and helper/error text with proper ARIA linkage
 * (`aria-describedby` → helper, `aria-invalid` → error).
 *
 * **Use directly only when you need a custom control** — textareas,
 * radio groups, control composites — where the standard `Input` and
 * `Select` primitives in `apps/web/src/components/ui/` don't apply.
 * Those already own their own label-and-error rendering, so wrapping
 * them in `FormField` would double up the label markup. This is the
 * documented escape-hatch shape (see `apps/web` for the canonical
 * `Input` / `Select` usage).
 *
 * The `children` render prop receives the id/aria props the control
 * must spread onto itself so screen readers announce label and
 * helper/error together.
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
