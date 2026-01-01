import { Sun, Moon } from 'lucide-react';
import { cn } from '@/lib/cn';
import { useThemeStore } from '@/stores/themeStore';

export interface ThemeToggleProps {
  className?: string;
}

export function ThemeToggle({ className }: ThemeToggleProps) {
  const { theme, toggleTheme } = useThemeStore();

  return (
    <button
      onClick={toggleTheme}
      className={cn(
        'p-2 rounded-lg transition-colors',
        'bg-[var(--bg-surface)] border border-[var(--border-default)]',
        'hover:bg-[var(--bg-hover)]',
        'focus-visible:outline-none focus-visible:ring-[3px]',
        'focus-visible:ring-[var(--ring-default)] focus-visible:ring-offset-2',
        'focus-visible:ring-offset-[var(--ring-offset)]',
        className
      )}
      aria-label={`Switch to ${theme === 'light' ? 'dark' : 'light'} mode`}
    >
      {theme === 'light' ? (
        <Moon className="w-5 h-5 text-[var(--text-secondary)]" />
      ) : (
        <Sun className="w-5 h-5 text-[var(--text-secondary)]" />
      )}
    </button>
  );
}
