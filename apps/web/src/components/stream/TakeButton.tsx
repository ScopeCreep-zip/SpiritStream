/**
 * Take Button
 * Triggers scene transition from Preview to Program in Studio Mode
 */
import { cn } from '@/lib/utils';

interface TakeButtonProps {
  onClick: () => void;
  disabled?: boolean;
  className?: string;
}

export function TakeButton({ onClick, disabled, className }: TakeButtonProps) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={cn(
        'w-16 h-16 rounded-lg font-bold text-sm transition-all',
        'bg-red-600 hover:bg-red-500 active:bg-red-700',
        'text-white shadow-lg',
        'disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-red-600',
        'focus:outline-none focus:ring-4 focus:ring-red-500/30',
        className
      )}
    >
      TAKE
    </button>
  );
}
