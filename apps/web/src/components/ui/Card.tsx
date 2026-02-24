import { cn } from '@/lib/cn';

export interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  variant?: 'default' | 'elevated' | 'interactive';
}

export function Card({ className, variant = 'default', ...props }: CardProps) {
  const variants = {
    default: 'bg-bg-surface shadow-sm',
    elevated: 'bg-bg-elevated shadow-md',
    interactive:
      'bg-bg-surface shadow-sm hover:shadow-md hover:border-border-interactive cursor-pointer transition-all',
  };

  return (
    <div
      className={cn(
        'rounded-xl border border-border-default',
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
        'border-b border-border-muted',
        'flex items-center justify-between',
        'py-5 px-6',
        className
      )}
      {...props}
    />
  );
}

export interface CardTitleProps extends React.HTMLAttributes<HTMLHeadingElement> {}

export function CardTitle({ className, ...props }: CardTitleProps) {
  return (
    <h3
      className={cn('text-base font-semibold text-text-primary', className)}
      {...props}
    />
  );
}

export interface CardDescriptionProps extends React.HTMLAttributes<HTMLParagraphElement> {}

export function CardDescription({ className, ...props }: CardDescriptionProps) {
  return <p className={cn('text-sm text-text-secondary mt-1', className)} {...props} />;
}

export interface CardBodyProps extends React.HTMLAttributes<HTMLDivElement> {}

export function CardBody({ className, ...props }: CardBodyProps) {
  return <div className={cn('p-6', className)} {...props} />;
}

export interface CardFooterProps extends React.HTMLAttributes<HTMLDivElement> {}

export function CardFooter({ className, ...props }: CardFooterProps) {
  return (
    <div
      className={cn(
        'border-t border-border-muted bg-bg-muted rounded-b-xl',
        'flex justify-end gap-3',
        'py-4 px-6',
        className
      )}
      {...props}
    />
  );
}
