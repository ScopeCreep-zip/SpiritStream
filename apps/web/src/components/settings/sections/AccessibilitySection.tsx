import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import { useHighContrast } from '@/hooks/useHighContrast';
import { useStoredHotkey } from '@/hooks/useStoredHotkey';
import { DEFAULT_PANIC_BINDING } from '@/hooks/usePanicHotkey';
import { formatBindingTokens, isMacPlatform } from '@/lib/hotkey';
import { KeybindCaptureDialog } from '@/components/settings/KeybindCaptureDialog';

/**
 * High-contrast toggle + panic hotkey rebind row. Saved per-device.
 * Pure self-contained — owns the capture-dialog open state and uses
 * useHighContrast + useStoredHotkey directly.
 */
export function AccessibilitySection() {
  const { t } = useTranslation();
  const { enabled: highContrast, toggle: toggleHighContrast } = useHighContrast();
  const { binding: panicBinding, setBinding: setPanicBinding } = useStoredHotkey(
    'panic',
    DEFAULT_PANIC_BINDING
  );
  const [captureOpen, setCaptureOpen] = useState(false);
  const isMac = isMacPlatform();
  const panicTokens = formatBindingTokens(panicBinding, isMac);

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle>{t('settings.accessibility', { defaultValue: 'Accessibility' })}</CardTitle>
          <CardDescription>
            {t('settings.accessibilityDescription', {
              defaultValue: 'Visual and motion preferences. Saved per-device.',
            })}
          </CardDescription>
        </div>
      </CardHeader>
      <CardBody className="p-6 flex flex-col gap-4">
        <div className="flex items-center justify-between py-2">
          <div>
            <div className="text-sm font-medium text-text-primary">
              {t('settings.highContrast', { defaultValue: 'High contrast' })}
            </div>
            <div className="text-xs text-text-tertiary">
              {t('settings.highContrastDescription', {
                defaultValue:
                  'Maximum-contrast palette (WCAG AAA, 7:1). Use if you have low vision or work in bright light.',
              })}
            </div>
          </div>
          <Toggle checked={highContrast} onChange={toggleHighContrast} />
        </div>
        <div className="flex items-center justify-between py-2 border-t border-border-muted pt-4">
          <div className="flex-1 min-w-0 pe-4">
            <div className="text-sm font-medium text-text-primary">
              {t('settings.panicHotkey', { defaultValue: 'Panic hotkey' })}
            </div>
            <div className="text-xs text-text-tertiary">
              {t('settings.panicHotkeyDescription', {
                defaultValue:
                  'Pressing this key stops every active stream immediately. Desktop builds bind it globally.',
              })}
            </div>
          </div>
          <div className="flex items-center gap-2 flex-shrink-0">
            <div className="flex items-center gap-1">
              {panicTokens.map((tok, i) => (
                <kbd
                  key={`panic-${i}`}
                  className="px-2 py-1 text-sm font-mono rounded border border-border-default bg-bg-muted text-text-secondary min-w-[2rem] text-center"
                >
                  {tok}
                </kbd>
              ))}
            </div>
            <Button variant="outline" size="sm" onClick={() => setCaptureOpen(true)}>
              {t('settings.changeHotkey', { defaultValue: 'Change…' })}
            </Button>
          </div>
        </div>
      </CardBody>
      <KeybindCaptureDialog
        open={captureOpen}
        onClose={() => setCaptureOpen(false)}
        onCommit={setPanicBinding}
        label={t('settings.panicHotkey', { defaultValue: 'Panic hotkey' })}
      />
    </Card>
  );
}
