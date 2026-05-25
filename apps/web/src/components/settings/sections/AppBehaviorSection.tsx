import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Toggle } from '@/components/ui/Toggle';
import { Select } from '@/components/ui/Select';
import { useSettings, useUpdateSetting } from '@/hooks/useSettings';
import type { Settings as AppSettings } from '@spiritstream/types';

/**
 * Global app-behavior toggles (start minimized) + log retention period.
 */
export function AppBehaviorSection() {
  const { t } = useTranslation();
  const { data: settings } = useSettings();
  const updateSettingMutation = useUpdateSetting();

  const logRetentionOptions = [
    { value: 7, label: t('settings.logRetention7Days', { defaultValue: '7 days' }) },
    { value: 14, label: t('settings.logRetention14Days', { defaultValue: '14 days' }) },
    { value: 30, label: t('settings.logRetention30Days', { defaultValue: '30 days' }) },
    { value: 90, label: t('settings.logRetention90Days', { defaultValue: '90 days' }) },
    { value: 365, label: t('settings.logRetention365Days', { defaultValue: '365 days' }) },
  ];

  const updateGlobalSetting = useCallback(
    <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
      updateSettingMutation.mutate({ key, value });
    },
    [updateSettingMutation],
  );

  if (!settings) return null;

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle>{t('settings.appBehavior', { defaultValue: 'App Behavior' })}</CardTitle>
          <CardDescription>
            {t('settings.appBehaviorDescription', {
              defaultValue: 'System-wide application settings.',
            })}
          </CardDescription>
        </div>
      </CardHeader>
      <CardBody className="p-6 flex flex-col gap-4">
        <div className="flex items-center justify-between py-2">
          <div>
            <div className="text-sm font-medium text-text-primary">
              {t('settings.startMinimized')}
            </div>
            <div className="text-xs text-text-tertiary">
              {t('settings.startMinimizedDescription')}
            </div>
          </div>
          <Toggle
            checked={settings.startMinimized}
            onChange={(checked: boolean) => updateGlobalSetting('startMinimized', checked)}
          />
        </div>
        <Select
          label={t('settings.logRetention', { defaultValue: 'Log retention' })}
          value={String(settings.logRetentionDays)}
          onChange={(e) => updateGlobalSetting('logRetentionDays', Number(e.target.value))}
          options={logRetentionOptions.map((option) => ({
            value: String(option.value),
            label: option.label,
          }))}
          helper={t('settings.logRetentionDescription', {
            defaultValue: 'How long to keep application log files.',
          })}
        />
      </CardBody>
    </Card>
  );
}
