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
    wrapper: 'bg-primary-muted border-primary text-primary',
    icon: Info,
  },
  success: {
    wrapper: 'bg-success-subtle border-success-border text-success-text',
    icon: CheckCircle,
  },
  warning: {
    wrapper: 'bg-warning-subtle border-warning-border text-warning-text',
    icon: AlertTriangle,
  },
  error: {
    wrapper: 'bg-error-subtle border-error-border text-error-text',
    icon: XCircle,
  },
};

export function Alert({ variant, title, children, className }: AlertProps) {
  const { wrapper, icon: Icon } = alertConfig[variant];

  return (
    <div className={cn('rounded-lg border flex gap-3 p-4 mb-4', wrapper, className)} role="alert">
      <Icon className="w-5 h-5 flex-shrink-0 mt-0.5" />
      <div className="flex-1 min-w-0">
        {title && <div className="font-semibold mb-1">{title}</div>}
        <div className="text-sm">{children}</div>
      </div>
    </div>
  );
}
