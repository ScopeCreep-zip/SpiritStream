import { cn } from '@/lib/cn';

export interface FormGroupProps extends React.HTMLAttributes<HTMLDivElement> {}

export function FormGroup({ className, ...props }: FormGroupProps) {
  return <div className={cn('mb-4', className)} {...props} />;
}

export interface FormLabelProps extends React.LabelHTMLAttributes<HTMLLabelElement> {}

export function FormLabel({ className, ...props }: FormLabelProps) {
  return (
    <label
      className={cn(
        'block mb-1.5 text-sm font-medium text-[var(--text-primary)]',
        className
      )}
      {...props}
    />
  );
}

export interface FormHelperProps extends React.HTMLAttributes<HTMLParagraphElement> {}

export function FormHelper({ className, ...props }: FormHelperProps) {
  return (
    <p
      className={cn('mt-1.5 text-xs text-[var(--text-tertiary)]', className)}
      {...props}
    />
  );
}

export interface FormErrorProps extends React.HTMLAttributes<HTMLParagraphElement> {}

export function FormError({ className, ...props }: FormErrorProps) {
  return (
    <p
      className={cn('mt-1.5 text-xs text-[var(--error-text)]', className)}
      {...props}
    />
  );
}

export interface FormRowProps extends React.HTMLAttributes<HTMLDivElement> {
  cols?: 2 | 3 | 4;
}

export function FormRow({ className, cols = 2, ...props }: FormRowProps) {
  const colStyles = {
    2: 'grid-cols-2',
    3: 'grid-cols-3',
    4: 'grid-cols-4',
  };

  return (
    <div
      className={cn('grid gap-4', colStyles[cols], className)}
      {...props}
    />
  );
}
