/**
 * Studio Mode Settings
 * Gear icon with popover for Studio Mode configuration
 */
import { useState, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useStudioStore } from '@/stores/studioStore';

export function StudioModeSettings() {
  const { t } = useTranslation();
  const { swapAfterTransition, setSwapAfterTransition } = useStudioStore();
  const [isOpen, setIsOpen] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);

  // Close popover when clicking outside
  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (
        popoverRef.current &&
        !popoverRef.current.contains(event.target as Node) &&
        buttonRef.current &&
        !buttonRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false);
      }
    }

    if (isOpen) {
      document.addEventListener('mousedown', handleClickOutside);
      return () => document.removeEventListener('mousedown', handleClickOutside);
    }
  }, [isOpen]);

  return (
    <div className="relative">
      <button
        ref={buttonRef}
        onClick={() => setIsOpen(!isOpen)}
        className="p-1.5 rounded hover:bg-[var(--bg-elevated)] text-[var(--text-muted)] hover:text-[var(--text-secondary)] transition-colors"
        title={t('stream.studioSettings', { defaultValue: 'Studio Settings' })}
        aria-expanded={isOpen}
        aria-haspopup="true"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <circle cx="12" cy="12" r="3" />
          <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
        </svg>
      </button>

      {isOpen && (
        <div
          ref={popoverRef}
          className="absolute top-full mt-1 left-1/2 -translate-x-1/2 z-50 w-48 bg-[var(--bg-elevated)] border border-[var(--border-default)] rounded-lg shadow-lg p-3"
        >
          <h4 className="text-xs font-medium text-[var(--text-primary)] mb-3">
            {t('stream.studioSettings', { defaultValue: 'Studio Settings' })}
          </h4>

          <label className="flex items-start gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={swapAfterTransition}
              onChange={(e) => setSwapAfterTransition(e.target.checked)}
              className="mt-0.5 h-4 w-4 rounded border-[var(--border-default)] bg-[var(--bg-sunken)] text-primary focus:ring-primary/50 cursor-pointer"
            />
            <span className="text-xs text-[var(--text-secondary)] leading-tight">
              {t('stream.swapAfterTransition', {
                defaultValue: 'Swap scenes after TAKE',
              })}
            </span>
          </label>

          <p className="mt-2 text-[10px] text-[var(--text-muted)] leading-tight">
            {t('stream.swapAfterTransitionDesc', {
              defaultValue:
                'After transition, the old Program scene becomes the new Preview scene.',
            })}
          </p>
        </div>
      )}
    </div>
  );
}
