import { X, CheckCircle, XCircle, Info } from 'lucide-react';
import { useToast, type Toast as ToastType } from '@/hooks/useToast';
import { cn } from '@/lib/cn';

function ToastItem({ toast }: { toast: ToastType }) {
  const { removeToast } = useToast();

  const icons = {
    success: <CheckCircle className="w-5 h-5" />,
    error: <XCircle className="w-5 h-5" />,
    info: <Info className="w-5 h-5" />,
  };

  const styles = {
    success: 'bg-success-subtle border-success-border text-success-text',
    error: 'bg-error-subtle border-error-border text-error-text',
    info: 'bg-primary-muted border-primary text-primary',
  };

  return (
    <div
      className={cn(
        'flex items-center gap-3 px-4 py-3 rounded-lg border shadow-lg',
        'animate-in slide-in-from-right-full duration-300',
        styles[toast.type]
      )}
    >
      {icons[toast.type]}
      <span className="flex-1 text-sm font-medium">{toast.message}</span>
      <button
        onClick={() => removeToast(toast.id)}
        className="p-1 rounded hover:bg-black/10 transition-colors"
      >
        <X className="w-4 h-4" />
      </button>
    </div>
  );
}

export function ToastContainer() {
  const { toasts } = useToast();

  if (toasts.length === 0) return null;

  return (
    // `end-4` is the logical equivalent of `right-4`; under
    // RTL the toast stack flips to the bottom-left where users in
    // RTL locales expect transient notifications. `z-[var(--z-toast)]`
    // uses the centralised ladder.
    <div
      role="region"
      aria-label="Notifications"
      aria-live="polite"
      className="fixed bottom-4 end-4 z-[var(--z-toast)] flex flex-col gap-2 max-w-sm"
    >
      {toasts.map((toast) => (
        <ToastItem key={toast.id} toast={toast} />
      ))}
    </div>
  );
}
