import React from 'react';
import { useTranslation } from 'react-i18next';
import { useStoredHotkey } from '@/hooks/useStoredHotkey';
import { DEFAULT_PANIC_BINDING } from '@/hooks/usePanicHotkey';
import { formatBindingTokens, isMacPlatform } from '@/lib/hotkey';

interface Shortcut {
  readonly id: string;
  readonly keys: ReadonlyArray<string>;
  readonly labelKey: string;
  readonly fallback: string;
}

/**
 * Keyboard-shortcut reference body — the `<dl>` of bindings. Rendered as a
 * section of the unified settings window (no Modal of its own). The panic key
 * is user-configurable and shown live; the rest are still hard-coded until
 * they gain rebinding via `useStoredHotkey`.
 */
export function ShortcutsList(): React.ReactElement {
  const { t } = useTranslation();
  const { binding: panicBinding } = useStoredHotkey('panic', DEFAULT_PANIC_BINDING);
  const isMac = isMacPlatform();
  const cmd = isMac ? '⌘' : 'Ctrl';

  const shortcuts: ReadonlyArray<Shortcut> = [
    {
      id: 'panic',
      keys: formatBindingTokens(panicBinding, isMac),
      labelKey: 'shortcuts.panic',
      fallback: 'Panic disconnect (stop all streams)',
    },
    {
      id: 'startStream',
      keys: [cmd, '↵'],
      labelKey: 'shortcuts.startStream',
      fallback: 'Start streaming',
    },
    {
      id: 'stopStream',
      keys: [cmd, '.'],
      labelKey: 'shortcuts.stopStream',
      fallback: 'Stop streaming',
    },
    {
      id: 'toggleChat',
      keys: [cmd, '\\'],
      labelKey: 'shortcuts.toggleChat',
      fallback: 'Show / hide chat panel',
    },
    { id: 'settings', keys: [cmd, ','], labelKey: 'shortcuts.settings', fallback: 'Open settings' },
    {
      id: 'shortcuts',
      keys: [cmd, '/'],
      labelKey: 'shortcuts.shortcuts',
      fallback: 'Show this shortcuts list',
    },
  ];

  return (
    <section aria-labelledby="settings-shortcuts-heading" className="space-y-4">
      <h2 id="settings-shortcuts-heading" className="text-lg font-semibold text-text-primary">
        {t('shortcuts.title', { defaultValue: 'Keyboard shortcuts' })}
      </h2>
      <dl className="divide-y divide-border-muted">
        {shortcuts.map((s) => (
          <div key={s.id} className="flex items-center justify-between py-3 first:pt-0 last:pb-0">
            <dt className="text-text-primary">{t(s.labelKey, { defaultValue: s.fallback })}</dt>
            <dd className="flex items-center gap-1">
              {s.keys.map((k, i) => (
                <kbd
                  key={`${s.id}-${i}`}
                  className="px-2 py-1 text-sm font-mono rounded border border-border-default bg-bg-muted text-text-secondary min-w-[2rem] text-center"
                >
                  {k}
                </kbd>
              ))}
            </dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
