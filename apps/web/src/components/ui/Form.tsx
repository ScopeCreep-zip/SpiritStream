import { cn } from '@/lib/cn';

export interface FormGroupProps extends React.HTMLAttributes<HTMLDivElement> {}

export function FormGroup({ className, ...props }: FormGroupProps) {
  return <div className={cn('mb-4', className)} {...props} />;
}

export interface FormLabelProps extends React.LabelHTMLAttributes<HTMLLabelElement> {}

export function FormLabel({ className, ...props }: FormLabelProps) {
  return (
    <label
      className={cn('block text-sm font-medium text-[var(--text-primary)] mb-1.5', className)}
      {...props}
    />
  );
}

export interface FormHelperProps extends React.HTMLAttributes<HTMLParagraphElement> {}

export function FormHelper({ className, ...props }: FormHelperProps) {
  return (
    <p
      className={cn('text-xs text-[var(--text-tertiary)] mt-1.5', className)}
      {...props}
    />
  );
}

export interface FormErrorProps extends React.HTMLAttributes<HTMLParagraphElement> {}

export function FormError({ className, ...props }: FormErrorProps) {
  return (
    <p
      className={cn('text-xs text-[var(--error-text)] mt-1.5', className)}
      {...props}
    />
  );
}

