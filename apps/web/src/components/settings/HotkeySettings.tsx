/**
 * Hotkey Settings Component
 * Displays and allows editing of keyboard shortcuts
 */
import { useTranslation } from 'react-i18next';
import { Keyboard, RotateCcw } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import { useHotkeyStore } from '@/stores/hotkeyStore';
import { getActionLabel, formatHotkeyBinding } from '@/types/hotkeys';

export function HotkeySettings() {
  const { t } = useTranslation();
  const { enabled, bindings, setEnabled, setBindingEnabled, resetToDefaults } = useHotkeyStore();

  // Group bindings by action for better display
  const groupedBindings = bindings.reduce(
    (acc, binding) => {
      if (!acc[binding.action]) {
        acc[binding.action] = [];
      }
      acc[binding.action].push(binding);
      return acc;
    },
    {} as Record<string, typeof bindings>
  );

  // Get unique actions in a logical order
  const actionOrder = [
    'take',
    'scene1',
    'scene2',
    'scene3',
    'scene4',
    'scene5',
    'scene6',
    'scene7',
    'scene8',
    'scene9',
    'escape',
    'toggleStudioMode',
    'toggleMute',
    'toggleStream',
  ];

  const orderedActions = actionOrder.filter((action) => groupedBindings[action]);

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Keyboard className="w-5 h-5" />
            {t('settings.keyboardShortcuts', { defaultValue: 'Keyboard Shortcuts' })}
          </CardTitle>
          <CardDescription>
            {t('settings.keyboardShortcutsDescription', {
              defaultValue: 'Configure global hotkeys for quick actions in the Stream view.',
            })}
          </CardDescription>
        </div>
      </CardHeader>
      <CardBody style={{ padding: '24px', display: 'flex', flexDirection: 'column', gap: '16px' }}>
        {/* Global enable toggle */}
        <div className="flex items-center justify-between" style={{ padding: '8px 0' }}>
          <div>
            <div className="text-sm font-medium text-[var(--text-primary)]">
              {t('settings.enableHotkeys', { defaultValue: 'Enable Hotkeys' })}
            </div>
            <div className="text-xs text-[var(--text-tertiary)]">
              {t('settings.enableHotkeysDescription', {
                defaultValue: 'When disabled, no keyboard shortcuts will work in the Stream view.',
              })}
            </div>
          </div>
          <Toggle checked={enabled} onChange={setEnabled} />
        </div>

        {/* Hotkey bindings table */}
        <div
          className={`border border-[var(--border-default)] rounded-lg overflow-hidden ${!enabled ? 'opacity-50 pointer-events-none' : ''}`}
        >
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-[var(--bg-elevated)] border-b border-[var(--border-default)]">
                <th className="px-4 py-2 text-left font-medium text-[var(--text-muted)]">
                  {t('settings.action', { defaultValue: 'Action' })}
                </th>
                <th className="px-4 py-2 text-left font-medium text-[var(--text-muted)]">
                  {t('settings.shortcut', { defaultValue: 'Shortcut' })}
                </th>
                <th className="px-4 py-2 text-center font-medium text-[var(--text-muted)]">
                  {t('settings.enabled', { defaultValue: 'Enabled' })}
                </th>
              </tr>
            </thead>
            <tbody>
              {orderedActions.map((action) => {
                const actionBindings = groupedBindings[action];
                return actionBindings.map((binding, index) => (
                  <tr
                    key={binding.id}
                    className="border-b border-[var(--border-default)] last:border-b-0 hover:bg-[var(--bg-hover)]"
                  >
                    <td className="px-4 py-2.5 text-[var(--text-primary)]">
                      {index === 0 ? getActionLabel(binding.action) : ''}
                    </td>
                    <td className="px-4 py-2.5">
                      <kbd className="inline-flex items-center px-2 py-1 rounded bg-[var(--bg-sunken)] border border-[var(--border-default)] font-mono text-xs text-[var(--text-secondary)]">
                        {formatHotkeyBinding(binding)}
                      </kbd>
                    </td>
                    <td className="px-4 py-2.5 text-center">
                      <Toggle
                        checked={binding.enabled}
                        onChange={(checked) => setBindingEnabled(binding.id, checked)}
                        size="sm"
                      />
                    </td>
                  </tr>
                ));
              })}
            </tbody>
          </table>
        </div>

        {/* Reset button */}
        <div className="flex justify-end pt-2">
          <Button variant="outline" size="sm" onClick={resetToDefaults}>
            <RotateCcw className="w-4 h-4" />
            {t('settings.resetToDefaults', { defaultValue: 'Reset to Defaults' })}
          </Button>
        </div>

        {/* Usage hint */}
        <div className="text-xs text-[var(--text-muted)] bg-[var(--bg-sunken)] rounded-lg p-3">
          <p>
            {t('settings.hotkeyHint', {
              defaultValue:
                'Hotkeys are active in the Stream view. They are disabled when typing in text fields.',
            })}
          </p>
        </div>
      </CardBody>
    </Card>
  );
}
