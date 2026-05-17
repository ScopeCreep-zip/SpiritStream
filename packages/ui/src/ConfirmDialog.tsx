import type { ReactNode } from 'react';

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
  if (!open) return null;

  const confirmClass =
    confirmVariant === 'danger'
      ? 'bg-error hover:bg-error-hover text-error-foreground'
      : 'bg-primary hover:bg-primary-hover text-primary-foreground';

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="confirm-dialog-title"
      className="fixed inset-0 z-[var(--z-modal)] flex items-center justify-center bg-bg-overlay"
      onClick={(e) => {
        if (e.target === e.currentTarget) onCancel();
      }}
    >
      <div className="bg-bg-surface rounded-xl shadow-xl max-w-md w-full mx-4 p-6 flex flex-col gap-4 animate-in zoom-in-95">
        <h2 id="confirm-dialog-title" className="text-lg font-semibold text-text-primary">
          {title}
        </h2>
        <div className="text-sm text-text-secondary">
          {typeof message === 'string' ? <p>{message}</p> : message}
        </div>
        <div className="flex justify-end gap-2 mt-2">
          <button
            type="button"
            onClick={onCancel}
            className="px-4 py-2 rounded-lg border border-border-default text-text-primary hover:bg-bg-hover transition-colors"
          >
            {cancelLabel}
          </button>
          <button
            type="button"
            onClick={onConfirm}
            disabled={confirmDisabled}
            className={`px-4 py-2 rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed ${confirmClass}`}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
