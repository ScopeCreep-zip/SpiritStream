import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/cn';

export interface AppShellProps {
  children: React.ReactNode;
  className?: string;
  /**
   * Anchor id for the skip-to-main-content link.
   * Defaults to "main-content"; pages can override if their landmark
   * has a different id.
   */
  mainContentId?: string;
}

export function AppShell({ children, className, mainContentId = 'main-content' }: AppShellProps) {
  const { t } = useTranslation();
  return (
    <div className={cn('flex min-h-screen bg-bg-base', className)}>
      {/* Skip-to-main-content. First focusable element so
          keyboard / screen-reader users can jump past the sidebar. */}
      <a href={`#${mainContentId}`} className="skip-link">
        {t('a11y.skipToMain', 'Skip to main content')}
      </a>
      {children}
    </div>
  );
}
