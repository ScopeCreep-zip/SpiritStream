import { cn } from '@/lib/cn';

export interface NavSectionProps {
  title: string;
  children: React.ReactNode;
  className?: string;
}

export function NavSection({ title, children, className }: NavSectionProps) {
  return (
    <div className={className} style={{ marginBottom: '24px' }}>
      <div
        className={cn(
          'text-tiny font-semibold uppercase tracking-wider',
          'text-[var(--text-tertiary)]'
        )}
        style={{ padding: '0 12px', marginBottom: '8px' }}
      >
        {title}
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>{children}</div>
    </div>
  );
}
