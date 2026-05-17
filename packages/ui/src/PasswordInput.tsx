import { forwardRef, useState, type ComponentProps } from 'react';

/**
 * Reveal-toggling password input. Wraps a standard `<input>`
 * and overlays an eye / eye-off button so the caller doesn't have to
 * re-implement the show-password affordance three times.
 *
 * Consolidates `LoginModal.tsx`, `ProfileModal.tsx`, and `TargetModal.tsx`
 * — each had a near-identical relative-container + show-toggle pattern.
 *
 * Renders against the same design tokens as the rest of the UI
 * (`apps/web/src/styles/tokens.css`). The host app must inject those
 * tokens (and any base styles); this primitive depends only on
 * `data-*` attributes + Tailwind utility classes wired in the host
 * stylesheet.
 */
export interface PasswordInputProps
  extends Omit<ComponentProps<'input'>, 'type'> {
  /**
   * Optional label rendered above the input. The `<label>` element
   * uses `htmlFor` so screen readers associate it correctly; callers
   * must pass an `id` if the input is reachable via label click.
   */
  label?: string;
  /** Translatable label for the visibility toggle (aria-label). */
  showLabel?: string;
  hideLabel?: string;
  /** Optional helper text rendered below the input. */
  helper?: string;
  /** Optional error message. Mutually exclusive with `helper`. */
  error?: string;
  /**
   * Render-prop for the visibility-toggle icon. Allows the host to
   * pass any icon library (lucide / heroicons / inline SVG) without
   * dragging that dependency into `packages/ui`.
   */
  renderToggleIcon: (visible: boolean) => React.ReactNode;
  /**
   * Optional controlled visibility. When supplied, the host owns the
   * `visible` state and must also pass `onVisibilityChange`. Use this
   * to share a single show/hide state across multiple inputs (e.g.
   * password + confirm-password fields). Omit for the common case
   * where the component manages its own state.
   */
  visible?: boolean;
  onVisibilityChange?: (visible: boolean) => void;
}

export const PasswordInput = forwardRef<HTMLInputElement, PasswordInputProps>(
  (
    {
      label,
      showLabel = 'Show',
      hideLabel = 'Hide',
      helper,
      error,
      renderToggleIcon,
      visible: visibleProp,
      onVisibilityChange,
      className,
      disabled,
      id,
      ...inputProps
    },
    ref,
  ) => {
    const [internalVisible, setInternalVisible] = useState(false);
    const isControlled = visibleProp !== undefined;
    const visible = isControlled ? visibleProp : internalVisible;
    const setVisible = (next: boolean) => {
      if (isControlled) {
        onVisibilityChange?.(next);
      } else {
        setInternalVisible(next);
      }
    };
    const describedById = helper || error ? `${id ?? 'pw'}-describedby` : undefined;
    return (
      <div className="relative">
        {label && (
          <label
            htmlFor={id}
            className="block text-sm font-medium text-text-primary mb-1.5"
          >
            {label}
          </label>
        )}
        <input
          ref={ref}
          id={id}
          type={visible ? 'text' : 'password'}
          aria-invalid={error ? true : undefined}
          aria-describedby={describedById}
          disabled={disabled}
          className={
            className ??
            'w-full px-3 py-2 bg-bg-sunken border border-border-default rounded-lg ' +
              'text-text-primary placeholder:text-text-muted ' +
              'focus:outline-none focus:ring-2 focus:ring-ring-default focus:border-border-interactive ' +
              'disabled:opacity-50 disabled:cursor-not-allowed'
          }
          {...inputProps}
        />
        <button
          type="button"
          onClick={() => setVisible(!visible)}
          aria-label={visible ? hideLabel : showLabel}
          aria-pressed={visible}
          disabled={disabled}
          className={
            'absolute right-3 ' +
            (label ? 'top-[34px]' : 'top-1/2 -translate-y-1/2') +
            ' text-text-tertiary hover:text-text-primary transition-colors ' +
            'disabled:opacity-50 disabled:cursor-not-allowed'
          }
        >
          {renderToggleIcon(visible)}
        </button>
        {(helper || error) && (
          <p
            id={describedById}
            className={
              error
                ? 'mt-1 text-xs text-error-text'
                : 'mt-1 text-xs text-text-tertiary'
            }
          >
            {error ?? helper}
          </p>
        )}
      </div>
    );
  },
);

PasswordInput.displayName = 'PasswordInput';
