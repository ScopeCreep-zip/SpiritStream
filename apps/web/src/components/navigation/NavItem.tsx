import { cn } from '@/lib/cn';
import { NavBadge } from './NavBadge';

export interface NavItemProps {
  icon: React.ReactNode;
  label: string;
  active?: boolean;
  badge?: number;
  onClick?: () => void;
  className?: string;
}

export function NavItem({ icon, label, active, badge, onClick, className }: NavItemProps) {
  return (
    <button
      onClick={onClick}
      className={cn(
        'w-full flex items-center gap-3 rounded-lg',
        'text-sm font-medium transition-all duration-150',
        'border-none bg-transparent text-left cursor-pointer',
        active
          ? 'bg-primary-subtle text-primary'
          : 'text-text-secondary hover:bg-bg-hover hover:text-text-primary',
        'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default',
        'py-2.5 px-3',
        className
      )}
    >
      <span className="w-5 h-5 flex items-center justify-center">{icon}</span>
      <span className="flex-1">{label}</span>
      {badge !== undefined && badge > 0 && <NavBadge count={badge} />}
    </button>
  );
}
