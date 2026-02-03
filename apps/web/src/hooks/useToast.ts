import { create } from 'zustand';
import { useSettingsStore } from '@/stores/settingsStore';

export type ToastType = 'success' | 'error' | 'info';

export interface Toast {
  id: string;
  type: ToastType;
  message: string;
}

interface ToastStore {
  toasts: Toast[];
  addToast: (type: ToastType, message: string) => void;
  removeToast: (id: string) => void;
}

export const useToast = create<ToastStore>((set) => ({
  toasts: [],
  addToast: (type, message) => {
    // Check if notifications are enabled (always show errors regardless)
    const { showNotifications } = useSettingsStore.getState();
    if (!showNotifications && type !== 'error') {
      return;
    }

    const id = crypto.randomUUID();
    set((state) => ({
      toasts: [...state.toasts, { id, type, message }],
    }));
    // Auto-remove after 3 seconds
    setTimeout(() => {
      set((state) => ({
        toasts: state.toasts.filter((t) => t.id !== id),
      }));
    }, 3000);
  },
  removeToast: (id) => {
    set((state) => ({
      toasts: state.toasts.filter((t) => t.id !== id),
    }));
  },
}));

// Helper functions for convenience
export const toast = {
  success: (message: string) => useToast.getState().addToast('success', message),
  error: (message: string) => useToast.getState().addToast('error', message),
  info: (message: string) => useToast.getState().addToast('info', message),
};

// ============================================================================
// Error Handling Utilities
// ============================================================================

/**
 * Format an error into a consistent string message
 */
export function formatError(err: unknown): string {
  if (err instanceof Error) {
    return err.message;
  }
  return String(err);
}

/**
 * Create a reusable error handler with i18n support
 *
 * @param translationFn - The i18n translation function (t)
 * @param errorKey - The translation key for the error message
 * @param defaultMessage - Fallback message if translation is missing
 * @returns A function that handles errors and shows toast
 *
 * @example
 * ```tsx
 * const { t } = useTranslation();
 * const handleError = createErrorHandler(t, 'stream.updateFailed', 'Failed to update');
 *
 * try {
 *   await updateSomething();
 * } catch (err) {
 *   handleError(err);
 * }
 * ```
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function createErrorHandler(
  translationFn: (key: any, options?: any) => unknown,
  errorKey: string,
  defaultMessage: string
): (err: unknown) => void {
  return (err: unknown) => {
    const errorMessage = formatError(err);
    toast.error(
      String(translationFn(errorKey, {
        error: errorMessage,
        defaultValue: `${defaultMessage}: ${errorMessage}`,
      }))
    );
  };
}

/**
 * Wrap an async function with error handling
 *
 * @param asyncFn - The async function to wrap
 * @param onError - Error handler function
 * @param onFinally - Optional cleanup function
 * @returns A wrapped function that catches errors and calls the handler
 *
 * @example
 * ```tsx
 * const handleError = createErrorHandler(t, 'stream.updateFailed', 'Failed to update');
 *
 * const safeUpdate = withErrorHandling(
 *   async () => await updateLayer(...),
 *   handleError,
 *   () => setIsLoading(false)
 * );
 *
 * await safeUpdate();
 * ```
 */
export function withErrorHandling<T extends unknown[], R>(
  asyncFn: (...args: T) => Promise<R>,
  onError: (err: unknown) => void,
  onFinally?: () => void
): (...args: T) => Promise<R | undefined> {
  return async (...args: T): Promise<R | undefined> => {
    try {
      return await asyncFn(...args);
    } catch (err) {
      onError(err);
      return undefined;
    } finally {
      onFinally?.();
    }
  };
}
