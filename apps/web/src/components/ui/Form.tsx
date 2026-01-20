import { cn } from '@/lib/cn';

export interface FormGroupProps extends React.HTMLAttributes<HTMLDivElement> {}

export function FormGroup({ className, ...props }: FormGroupProps) {
  return <div className={className} style={{ marginBottom: '16px' }} {...props} />;
}

export interface FormLabelProps extends React.LabelHTMLAttributes<HTMLLabelElement> {}

export function FormLabel({ className, ...props }: FormLabelProps) {
  return (
    <label
      className={cn('block text-sm font-medium text-[var(--text-primary)]', className)}
      style={{ marginBottom: '6px' }}
      {...props}
    />
  );
}

export interface FormHelperProps extends React.HTMLAttributes<HTMLParagraphElement> {}

export function FormHelper({ className, ...props }: FormHelperProps) {
  return (
    <p
      className={cn('text-xs text-[var(--text-tertiary)]', className)}
      style={{ marginTop: '6px' }}
      {...props}
    />
  );
}

export interface FormErrorProps extends React.HTMLAttributes<HTMLParagraphElement> {}

export function FormError({ className, ...props }: FormErrorProps) {
  return (
    <p
      className={cn('text-xs text-[var(--error-text)]', className)}
      style={{ marginTop: '6px' }}
      {...props}
    />
  );
}

