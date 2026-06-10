import { useEffect, useId, useRef, type ReactNode } from 'react';

/**
 * Destructive-operation confirmation dialog.
 *
 * Pair with the confirm-token endpoint:
 *
 * 1. User clicks "Clear data" → host calls
 *    `POST /api/v1/security/confirm-token { intent: "clear_data" }`
 *    to obtain a single-use token.
 * 2. Host opens `<ConfirmDialog>` to surface a final "are you sure?"
 *    prompt.
 * 3. On confirm, host sends the destructive request with
 *    `X-Confirm-Token: <token>`.
 *
 * This component is presentation-only — it does not interact with the
 * network. Wiring the token issue / consume flow stays in the host.
 *
 * A11y: Escape closes (Cancel). Initial focus moves to the Cancel
 * button — a destructive primary button should never be the first
 * focus target, since that lets a stray Enter keypress trigger the
 * destructive action. Focus is restored to the previously-focused
 * element on close. Focus is trapped inside the dialog while open.
 */
export interface ConfirmDialogProps {
  open: boolean;
  /** Human-readable headline (e.g., "Clear all data?"). */
  title: string;
  /**
   * Body content. Strings render as a single paragraph; nodes are
   * inserted verbatim so the host can layer warnings.
   */
  message: ReactNode;
  /**
   * Label on the destructive button. Defaults to "Delete" so the
   * caller's intent is loud.
   */
  confirmLabel?: string;
  /**
   * Label on the cancel button. Defaults to "Cancel".
   */
  cancelLabel?: string;
  /**
   * Variant of the confirm button.
   *  - `danger`  — red, used for destructive actions (default).
   *  - `primary` — violet, used for confirming a non-destructive action.
   */
  confirmVariant?: 'danger' | 'primary';
  /**
   * If true, the confirm button is disabled. The host can use this to
   * gate confirmation on the user re-typing a passphrase or similar.
   */
  confirmDisabled?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDialog({
  open,
  title,
  message,
  confirmLabel = 'Delete',
  cancelLabel = 'Cancel',
  confirmVariant = 'danger',
  confirmDisabled,
  onConfirm,
  onCancel,
}: ConfirmDialogProps): React.ReactElement | null {
  // useId returns a stable, render-consistent id even when multiple
  // dialogs are open — replaces the hard-coded `confirm-dialog-title`
  // which collided in that case.
  const titleId = useId();
  const cancelRef = useRef<HTMLButtonElement | null>(null);
  const confirmRef = useRef<HTMLButtonElement | null>(null);
  const previousActive = useRef<HTMLElement | null>(null);

  // Auto-focus cancel + restore previous focus on close. Mirrors the
  // pattern in `apps/web/src/components/ui/Modal.tsx` so keyboard
  // navigation works the same across both modal shapes.
  useEffect(() => {
    if (!open) return;
    previousActive.current = document.activeElement as HTMLElement | null;
    // requestAnimationFrame to let the new DOM paint before grabbing focus —
    // otherwise the focus call can race the React commit on slow devices.
    const raf = requestAnimationFrame(() => {
      cancelRef.current?.focus();
    });
    return () => {
      cancelAnimationFrame(raf);
      const prev = previousActive.current;
      if (prev && document.body.contains(prev)) {
        prev.focus();
      } else {
        document.body.focus();
      }
    };
  }, [open]);

  if (!open) return null;

  const confirmClass =
    confirmVariant === 'danger'
      ? 'bg-error hover:bg-error-hover text-error-foreground'
      : 'bg-primary hover:bg-primary-hover text-primary-foreground';

  // Trap focus between the two buttons. Tab from confirm wraps to
  // cancel; Shift+Tab from cancel wraps to confirm. This is the
  // minimum focus-trap for a two-button dialog; the full body content
  // is non-interactive prose so there's nothing else to cycle through.
  const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>): void => {
    if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
      return;
    }
    if (e.key !== 'Tab') return;
    const focusable = [cancelRef.current, confirmRef.current].filter(
      (el): el is HTMLButtonElement => el !== null && !el.disabled
    );
    if (focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      className="fixed inset-0 z-[var(--z-modal)] flex items-center justify-center bg-bg-overlay"
      onClick={(e) => {
        if (e.target === e.currentTarget) onCancel();
      }}
      onKeyDown={handleKeyDown}
    >
      <div className="bg-bg-surface rounded-xl shadow-xl max-w-md w-full mx-4 p-6 flex flex-col gap-4 animate-in zoom-in-95">
        <h2 id={titleId} className="text-lg font-semibold text-text-primary">
          {title}
        </h2>
        <div className="text-sm text-text-secondary">
          {typeof message === 'string' ? <p>{message}</p> : message}
        </div>
        <div className="flex justify-end gap-2 mt-2">
          <button
            ref={cancelRef}
            type="button"
            onClick={onCancel}
            className="px-4 py-2 rounded-lg border border-border-default text-text-primary hover:bg-bg-hover focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default transition-colors"
          >
            {cancelLabel}
          </button>
          <button
            ref={confirmRef}
            type="button"
            onClick={onConfirm}
            disabled={confirmDisabled}
            className={`px-4 py-2 rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default ${confirmClass}`}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
