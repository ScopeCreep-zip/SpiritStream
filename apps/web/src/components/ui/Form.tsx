import { cn } from '@/lib/cn';

// The form-field scaffold used by `Input` / `Select` and by the
// standalone-label forms (where a label sits next to a button so the
// input's built-in `label` prop can't be used). Spacing is gap-based
// (the `FormGroup` wrapper owns the rhythm); the label/helper/error
// carry no margins of their own, matching the rest of the design
// system.

export interface FormGroupProps extends React.HTMLAttributes<HTMLDivElement> {}

export function FormGroup({ className, ...props }: FormGroupProps) {
  return <div className={cn('flex flex-col gap-1.5', className)} {...props} />;
}

export interface FormLabelProps extends React.LabelHTMLAttributes<HTMLLabelElement> {}

export function FormLabel({ className, ...props }: FormLabelProps) {
  return (
    <label className={cn('block text-sm font-medium text-text-primary', className)} {...props} />
  );
}

export interface FormHelperProps extends React.HTMLAttributes<HTMLParagraphElement> {}

export function FormHelper({ className, ...props }: FormHelperProps) {
  return <p className={cn('text-xs text-text-tertiary', className)} {...props} />;
}

export interface FormErrorProps extends React.HTMLAttributes<HTMLParagraphElement> {}

export function FormError({ className, ...props }: FormErrorProps) {
  return <p role="alert" className={cn('text-xs text-error-text', className)} {...props} />;
}
