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
    <div className={cn('grid', colStyles[cols], className)} style={{ gap: '16px' }} {...props} />
  );
}
