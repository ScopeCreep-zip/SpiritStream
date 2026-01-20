import { useEffect, useRef, useCallback } from 'react';
import { X } from 'lucide-react';
import { cn } from '@/lib/cn';

export interface ModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: React.ReactNode;
  footer?: React.ReactNode;
  maxWidth?: string;
}

export function Modal({ open, onClose, title, children, footer, maxWidth = '500px' }: ModalProps) {
  const modalRef = useRef<HTMLDivElement>(null);
  const previousActiveElement = useRef<HTMLElement | null>(null);

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
      // Restore focus to previously focused element
      if (previousActiveElement.current) {
        previousActiveElement.current.focus();
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
        'fixed inset-0 z-[1000] flex items-center justify-center',
        'bg-[var(--bg-overlay)]',
        'animate-in fade-in duration-200'
      )}
      onClick={(e) => e.target === e.currentTarget && onClose()}
    >
      <div
        ref={modalRef}
        className={cn(
          'bg-[var(--bg-surface)] rounded-xl shadow-[var(--shadow-xl)]',
          'w-full max-h-[90vh] overflow-hidden',
          'animate-in zoom-in-95 duration-200',
          'flex flex-col'
        )}
        style={{ maxWidth }}
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
  return (
    <div
      className="flex-shrink-0 border-b border-[var(--border-muted)] flex items-center justify-between"
      style={{ padding: '20px 24px' }}
    >
      <h3 id="modal-title" className="text-lg font-semibold text-[var(--text-primary)]">
        {title}
      </h3>
      <button
        onClick={onClose}
        className={cn(
          'w-8 h-8 flex items-center justify-center rounded-md',
          'text-[var(--text-tertiary)] bg-transparent border-none cursor-pointer',
          'hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]',
          'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-[var(--ring-default)]',
          'transition-all duration-150'
        )}
        aria-label="Close modal"
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
  return (
    <div className={cn('flex-1 min-h-0 overflow-y-auto', className)} style={{ padding: '24px' }}>
      {children}
    </div>
  );
}

interface ModalFooterProps {
  children: React.ReactNode;
  className?: string;
}

export function ModalFooter({ children, className }: ModalFooterProps) {
  return (
    <div
      className={cn('flex-shrink-0 border-t border-[var(--border-muted)] flex justify-end gap-3', className)}
      style={{ padding: '16px 24px' }}
    >
      {children}
    </div>
  );
}
