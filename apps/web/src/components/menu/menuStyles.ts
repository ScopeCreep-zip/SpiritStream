import { cn } from '@/lib/cn';
import type { Language } from '@/stores/languageStore';

/**
 * Shared Tailwind class strings for the Radix Menubar surfaces. Lives
 * separately so every per-menu component renders an identical trigger /
 * content / item / separator visual — the MenuBar split preserves byte-
 * identical menu appearance.
 */

export const TRIGGER_CLASS = cn(
  'px-3 py-1.5 rounded-md text-sm font-medium text-text-secondary',
  'hover:bg-bg-hover hover:text-text-primary',
  'data-[state=open]:bg-bg-hover data-[state=open]:text-text-primary',
  'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default',
  'transition-colors'
);

export const CONTENT_CLASS = cn(
  'min-w-[220px] bg-bg-elevated border border-border-default rounded-md shadow-xl',
  'py-1 z-[var(--z-dropdown)]'
);

export const ITEM_CLASS = cn(
  'flex items-center justify-between gap-4 px-3 py-1.5 text-sm text-text-primary',
  'cursor-pointer select-none outline-none',
  'data-[highlighted]:bg-bg-hover',
  'data-[disabled]:text-text-disabled data-[disabled]:cursor-default data-[disabled]:bg-transparent'
);

export const SHORTCUT_CLASS = 'text-xs text-text-muted ms-auto ps-4';
export const SEPARATOR_CLASS = 'h-px my-1 bg-border-muted';

export const SUPPORTED_LANGUAGES: ReadonlyArray<{ code: Language; label: string }> = [
  { code: 'en', label: 'English' },
  { code: 'de', label: 'Deutsch' },
  { code: 'es', label: 'Español' },
  { code: 'fr', label: 'Français' },
  { code: 'af', label: 'Afrikaans' },
  { code: 'ar', label: 'العربية' },
  { code: 'ja', label: '日本語' },
  { code: 'ko', label: '한국어' },
  { code: 'ru', label: 'Русский' },
  { code: 'uk', label: 'Українська' },
  { code: 'zh-CN', label: '中文' },
];
