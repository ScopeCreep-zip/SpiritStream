import { memo } from 'react';
import { cn } from '@/lib/cn';

export interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  variant?: 'default' | 'elevated' | 'interactive';
}

const VARIANTS: Record<string, string> = {
  default: 'bg-[var(--bg-surface)] shadow-[var(--shadow-sm)]',
  elevated: 'bg-[var(--bg-elevated)] shadow-[var(--shadow-md)]',
  interactive:
    'bg-[var(--bg-surface)] shadow-[var(--shadow-sm)] hover:shadow-[var(--shadow-md)] hover:border-[var(--border-interactive)] cursor-pointer transition-shadow',
};

export const Card = memo(function Card({ className, variant = 'default', ...props }: CardProps) {
  return (
    <div
      className={cn(
        'rounded-xl border border-[var(--border-default)]',
        VARIANTS[variant],
        className
      )}
      {...props}
    />
  );
});

export interface CardHeaderProps extends React.HTMLAttributes<HTMLDivElement> {}

export const CardHeader = memo(function CardHeader({ className, ...props }: CardHeaderProps) {
  return (
    <div
      className={cn(
        'border-b border-[var(--border-muted)]',
        'flex items-center justify-between',
        'px-6 py-5',
        className
      )}
      {...props}
    />
  );
});

export interface CardTitleProps extends React.HTMLAttributes<HTMLHeadingElement> {}

export const CardTitle = memo(function CardTitle({ className, ...props }: CardTitleProps) {
  return (
    <h3
      className={cn('text-base font-semibold text-[var(--text-primary)]', className)}
      {...props}
    />
  );
});

export interface CardDescriptionProps extends React.HTMLAttributes<HTMLParagraphElement> {}

export const CardDescription = memo(function CardDescription({ className, ...props }: CardDescriptionProps) {
  return <p className={cn('text-sm text-[var(--text-secondary)] mt-1', className)} {...props} />;
});

export interface CardBodyProps extends React.HTMLAttributes<HTMLDivElement> {}

export const CardBody = memo(function CardBody({ className, ...props }: CardBodyProps) {
  return <div className={cn('p-6', className)} {...props} />;
});

export interface CardFooterProps extends React.HTMLAttributes<HTMLDivElement> {}

export const CardFooter = memo(function CardFooter({ className, ...props }: CardFooterProps) {
  return (
    <div
      className={cn(
        'border-t border-[var(--border-muted)] bg-[var(--bg-muted)] rounded-b-xl',
        'flex justify-end gap-3',
        'px-6 py-4',
        className
      )}
      {...props}
    />
  );
});
