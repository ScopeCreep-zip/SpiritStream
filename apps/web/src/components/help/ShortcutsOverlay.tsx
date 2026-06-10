import React from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { useStoredHotkey } from '@/hooks/useStoredHotkey';
import { DEFAULT_PANIC_BINDING } from '@/hooks/usePanicHotkey';
import { formatBindingTokens, isMacPlatform } from '@/lib/hotkey';

interface Shortcut {
  readonly id: string;
  readonly keys: ReadonlyArray<string>;
  readonly labelKey: string;
  readonly fallback: string;
}

interface ShortcutsOverlayProps {
  open: boolean;
  onClose: () => void;
}

export function ShortcutsOverlay({ open, onClose }: ShortcutsOverlayProps): React.ReactElement {
  const { t } = useTranslation();
  const { binding: panicBinding } = useStoredHotkey('panic', DEFAULT_PANIC_BINDING);
  const isMac = isMacPlatform();
  const cmd = isMac ? '⌘' : 'Ctrl';

  // Panic key is user-configurable; render the live binding. Everything else
  // is still hard-coded — those will gain rebinding when they land in
  // `useStoredHotkey`.
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
    <Modal
      open={open}
      onClose={onClose}
      title={t('shortcuts.title', { defaultValue: 'Keyboard shortcuts' })}
      maxWidth="520px"
      closeOnBackdropClick
    >
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
    </Modal>
  );
}
