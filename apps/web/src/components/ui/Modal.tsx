import { useEffect, useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { X } from 'lucide-react';
import { cn } from '@/lib/cn';

// Module-level stack of open modals. Escape and the Tab focus trap only
// act on the TOPMOST modal — without this, stacked modals (a confirm
// dialog over a settings modal) all closed on a single Escape press.
const modalStack: symbol[] = [];

export interface ModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: React.ReactNode;
  footer?: React.ReactNode;
  maxWidth?: string;
  /** Whether clicking the backdrop closes the modal. Default: false */
  closeOnBackdropClick?: boolean;
}

export function Modal({
  open,
  onClose,
  title,
  children,
  footer,
  maxWidth = '500px',
  closeOnBackdropClick = false,
}: ModalProps) {
  const modalRef = useRef<HTMLDivElement>(null);
  const previousActiveElement = useRef<HTMLElement | null>(null);
  const stackId = useRef<symbol>(Symbol('modal'));

  // Register on the modal stack while open.
  useEffect(() => {
    if (!open) return;
    const id = stackId.current;
    modalStack.push(id);
    return () => {
      const index = modalStack.indexOf(id);
      if (index !== -1) modalStack.splice(index, 1);
    };
  }, [open]);

  // Focus trap - get all focusable elements
  const getFocusableElements = useCallback(() => {
    if (!modalRef.current) return [];
    return Array.from(
      modalRef.current.querySelectorAll<HTMLElement>(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
      )
    ).filter((el) => !el.hasAttribute('disabled'));
  }, []);

  // Handle escape key and focus trap
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (!open) return;
      // Only the topmost modal in the stack handles keys.
      if (modalStack[modalStack.length - 1] !== stackId.current) return;

      if (e.key === 'Escape') {
        onClose();
        return;
      }

      // Focus trap with Tab key
      if (e.key === 'Tab') {
        const focusable = getFocusableElements();
        if (focusable.length === 0) return;

        const firstElement = focusable[0];
        const lastElement = focusable[focusable.length - 1];

        if (e.shiftKey) {
          // Shift + Tab
          if (document.activeElement === firstElement) {
            e.preventDefault();
            lastElement.focus();
          }
        } else {
          // Tab
          if (document.activeElement === lastElement) {
            e.preventDefault();
            firstElement.focus();
          }
        }
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [open, onClose, getFocusableElements]);

  // Prevent body scroll and manage focus when modal opens/closes
  useEffect(() => {
    if (open) {
      // Save currently focused element
      previousActiveElement.current = document.activeElement as HTMLElement;
      document.body.style.overflow = 'hidden';

      // Focus first focusable element in modal
      requestAnimationFrame(() => {
        const focusable = getFocusableElements();
        if (focusable.length > 0) {
          focusable[0].focus();
        }
      });
    } else {
      document.body.style.overflow = '';
      // Restore focus to previously focused element. If that element has
      // been removed from the DOM (e.g. modal was opened to delete the
      // very row that triggered it), `focus()` is a no-op and focus
      // falls to <body> — tab order resets to the top of the page.
      // Detect detachment and fall back to <body> explicitly so screen
      // readers and keyboard users get a predictable landing point.
      const prev = previousActiveElement.current;
      if (prev && document.body.contains(prev)) {
        prev.focus();
      } else {
        document.body.focus();
      }
    }
    return () => {
      document.body.style.overflow = '';
    };
  }, [open, getFocusableElements]);

  if (!open) return null;

  return (
    <div
      className={cn(
        // z-index from the centralised ladder.
        'fixed inset-0 z-[var(--z-modal-overlay)] flex items-center justify-center',
        'bg-bg-overlay',
        'animate-in fade-in duration-200'
      )}
      onClick={(e) => closeOnBackdropClick && e.target === e.currentTarget && onClose()}
    >
      <div
        ref={modalRef}
        className={cn(
          'bg-bg-surface rounded-xl shadow-xl',
          // Width comes from the `--modal-max-width` custom
          // property injected below. Dynamic, but the JS only ferries
          // a value; the styling rule lives in CSS.
          'w-full max-w-[var(--modal-max-width)] max-h-[90vh] overflow-hidden',
          'animate-in zoom-in-95 duration-200',
          'flex flex-col'
        )}
        style={{ '--modal-max-width': maxWidth } as React.CSSProperties}
        role="dialog"
        aria-modal="true"
        aria-labelledby="modal-title"
      >
        <ModalHeader title={title} onClose={onClose} />
        <ModalBody>{children}</ModalBody>
        {footer && <ModalFooter>{footer}</ModalFooter>}
      </div>
    </div>
  );
}

interface ModalHeaderProps {
  title: string;
  onClose: () => void;
}

export function ModalHeader({ title, onClose }: ModalHeaderProps) {
  const { t } = useTranslation();
  return (
    <div className="flex-shrink-0 border-b border-border-muted flex items-center justify-between py-5 px-6">
      <h3 id="modal-title" className="text-lg font-semibold text-text-primary">
        {title}
      </h3>
      <button
        onClick={onClose}
        className={cn(
          'w-8 h-8 flex items-center justify-center rounded-md',
          'text-text-tertiary bg-transparent border-none cursor-pointer',
          'hover:bg-bg-hover hover:text-text-primary',
          'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default',
          'transition-all duration-150'
        )}
        aria-label={t('common.close')}
      >
        <X className="w-4 h-4" />
      </button>
    </div>
  );
}

interface ModalBodyProps {
  children: React.ReactNode;
  className?: string;
}

export function ModalBody({ children, className }: ModalBodyProps) {
  return <div className={cn('flex-1 min-h-0 overflow-y-auto p-6', className)}>{children}</div>;
}

interface ModalFooterProps {
  children: React.ReactNode;
  className?: string;
}

export function ModalFooter({ children, className }: ModalFooterProps) {
  return (
    <div
      className={cn(
        'flex-shrink-0 border-t border-border-muted flex justify-end gap-3 py-4 px-6',
        className
      )}
    >
      {children}
    </div>
  );
}
