import { cn } from '@/lib/cn';

export interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  variant?: 'default' | 'elevated' | 'interactive';
}

export function Card({ className, variant = 'default', ...props }: CardProps) {
  const variants = {
    default: 'bg-[var(--bg-surface)] shadow-[var(--shadow-sm)]',
    elevated: 'bg-[var(--bg-elevated)] shadow-[var(--shadow-md)]',
    interactive: 'bg-[var(--bg-surface)] shadow-[var(--shadow-sm)] hover:shadow-[var(--shadow-md)] hover:border-[var(--border-interactive)] cursor-pointer transition-all',
  };

  return (
    <div
      className={cn(
        'rounded-xl border border-[var(--border-default)]',
        variants[variant],
        className
      )}
      {...props}
    />
  );
}

export interface CardHeaderProps extends React.HTMLAttributes<HTMLDivElement> {}

export function CardHeader({ className, ...props }: CardHeaderProps) {
  return (
    <div
      className={cn(
        'border-b border-[var(--border-muted)]',
        'flex items-center justify-between',
        className
      )}
      style={{ padding: '20px 24px' }}
      {...props}
    />
  );
}

export interface CardTitleProps extends React.HTMLAttributes<HTMLHeadingElement> {}

export function CardTitle({ className, ...props }: CardTitleProps) {
  return (
    <h3
      className={cn('text-base font-semibold text-[var(--text-primary)]', className)}
      {...props}
    />
  );
}

export interface CardDescriptionProps extends React.HTMLAttributes<HTMLParagraphElement> {}

export function CardDescription({ className, ...props }: CardDescriptionProps) {
  return (
    <p
      className={cn('text-sm text-[var(--text-secondary)] mt-1', className)}
      {...props}
    />
  );
}

export interface CardBodyProps extends React.HTMLAttributes<HTMLDivElement> {}

export function CardBody({ className, ...props }: CardBodyProps) {
  return <div className={className} style={{ padding: '24px' }} {...props} />;
}

export interface CardFooterProps extends React.HTMLAttributes<HTMLDivElement> {}

export function CardFooter({ className, ...props }: CardFooterProps) {
  return (
    <div
      className={cn(
        'border-t border-[var(--border-muted)] bg-[var(--bg-muted)] rounded-b-xl',
        'flex justify-end gap-3',
        className
      )}
      style={{ padding: '16px 24px' }}
      {...props}
    />
  );
}
