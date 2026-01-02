import { Info, CheckCircle, AlertTriangle, XCircle } from 'lucide-react';
import { cn } from '@/lib/cn';

export type AlertVariant = 'info' | 'success' | 'warning' | 'error';

export interface AlertProps {
  variant: AlertVariant;
  title?: string;
  children: React.ReactNode;
  className?: string;
}

const alertConfig = {
  info: {
    wrapper: 'bg-[var(--primary-muted)] border-[var(--primary)] text-[var(--primary)]',
    icon: Info,
  },
  success: {
    wrapper: 'bg-[var(--success-subtle)] border-[var(--success-border)] text-[var(--success-text)]',
    icon: CheckCircle,
  },
  warning: {
    wrapper: 'bg-[var(--warning-subtle)] border-[var(--warning-border)] text-[var(--warning-text)]',
    icon: AlertTriangle,
  },
  error: {
    wrapper: 'bg-[var(--error-subtle)] border-[var(--error-border)] text-[var(--error-text)]',
    icon: XCircle,
  },
};

export function Alert({ variant, title, children, className }: AlertProps) {
  const { wrapper, icon: Icon } = alertConfig[variant];

  return (
    <div
      className={cn(
        'rounded-lg border flex gap-3',
        wrapper,
        className
      )}
      style={{ padding: '16px', marginBottom: '16px' }}
      role="alert"
    >
      <Icon className="w-5 h-5 flex-shrink-0 mt-0.5" />
      <div className="flex-1 min-w-0">
        {title && <div className="font-semibold mb-1">{title}</div>}
        <div className="text-sm">{children}</div>
      </div>
    </div>
  );
}
