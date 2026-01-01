import { useEffect } from 'react';
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

export function Modal({
  open,
  onClose,
  title,
  children,
  footer,
  maxWidth = '500px',
}: ModalProps) {
  // Handle escape key
  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && open) {
        onClose();
      }
    };

    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [open, onClose]);

  // Prevent body scroll when modal is open
  useEffect(() => {
    if (open) {
      document.body.style.overflow = 'hidden';
    } else {
      document.body.style.overflow = '';
    }
    return () => {
      document.body.style.overflow = '';
    };
  }, [open]);

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
        className={cn(
          'bg-[var(--bg-surface)] rounded-xl shadow-[var(--shadow-xl)]',
          'w-full max-h-[90vh] overflow-hidden',
          'animate-in zoom-in-95 duration-200'
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
    <div className="px-6 py-5 border-b border-[var(--border-muted)] flex items-center justify-between">
      <h3
        id="modal-title"
        className="text-lg font-semibold text-[var(--text-primary)]"
      >
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
    <div className={cn('p-6 overflow-y-auto', className)}>
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
      className={cn(
        'px-6 py-4 border-t border-[var(--border-muted)] flex justify-end gap-3',
        className
      )}
    >
      {children}
    </div>
  );
}
