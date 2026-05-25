import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Eye, EyeOff } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Toggle } from '@/components/ui/Toggle';
import { Input } from '@/components/ui/Input';
import { PasswordInput } from '@spiritstream/ui';
import { useProfileStore } from '@/stores/profileStore';
import type { BackendSettings } from '@spiritstream/types';

/**
 * Remote-access (HTTP API) backend configuration for the active profile.
 * Spans both tabs of the Settings UI (col-span-2 in the original layout).
 */
export function RemoteAccessSection() {
  const { t } = useTranslation();
  const currentProfile = useProfileStore((state) => state.current);
  const updateProfileSettings = useProfileStore((state) => state.updateProfileSettings);
  const profileSettings = currentProfile?.settings;

  const updateBackendSetting = useCallback(
    async <K extends keyof BackendSettings>(key: K, value: BackendSettings[K]) => {
      if (!profileSettings) return;
      await updateProfileSettings({
        backend: { ...profileSettings.backend, [key]: value },
      });
    },
    [profileSettings, updateProfileSettings],
  );

  return (
    <Card className="col-span-2">
      <CardHeader>
        <div>
          <CardTitle>{t('settings.remoteAccess', { defaultValue: 'Remote Access' })}</CardTitle>
          <CardDescription>
            {t('settings.remoteAccessDescription', {
              defaultValue:
                'Enable the built-in HTTP API so you can manage SpiritStream from another device.',
            })}
          </CardDescription>
        </div>
      </CardHeader>
      <CardBody className="p-6 flex flex-col gap-4">
        <div className="grid grid-cols-2 gap-6">
          <div className="space-y-4">
            <div className="flex items-center justify-between py-2">
              <div>
                <div className="text-sm font-medium text-text-primary">
                  {t('settings.remoteAccessToggle', { defaultValue: 'Allow remote web access' })}
                </div>
                <div className="text-xs text-text-tertiary">
                  {t('settings.remoteAccessToggleDescription', {
                    defaultValue:
                      'When off, the API binds to localhost only. Restart required after changes.',
                  })}
                </div>
              </div>
              <Toggle
                checked={profileSettings?.backend?.remoteEnabled ?? false}
                onChange={(checked: boolean) => updateBackendSetting('remoteEnabled', checked)}
              />
            </div>
            <div className="flex items-center justify-between py-2">
              <div>
                <div className="text-sm font-medium text-text-primary">
                  {t('settings.remoteAccessUiToggle', {
                    defaultValue: 'Serve web GUI from the host',
                  })}
                </div>
                <div className="text-xs text-text-tertiary">
                  {t('settings.remoteAccessUiToggleDescription', {
                    defaultValue:
                      'When off, the host will not serve the UI files. Restart required after changes.',
                  })}
                </div>
              </div>
              <Toggle
                checked={profileSettings?.backend?.uiEnabled ?? false}
                onChange={(checked: boolean) => updateBackendSetting('uiEnabled', checked)}
              />
            </div>
          </div>
          <div className="space-y-4">
            <Input
              label={t('settings.remoteAccessHost', { defaultValue: 'Bind host' })}
              value={profileSettings?.backend?.host ?? '127.0.0.1'}
              onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                updateBackendSetting('host', e.target.value)
              }
              helper={t('settings.remoteAccessHostHelper', {
                defaultValue: 'Use 0.0.0.0 to listen on all interfaces.',
              })}
            />
            <Input
              label={t('settings.remoteAccessPort', { defaultValue: 'Port' })}
              type="number"
              value={profileSettings?.backend?.port ?? 8008}
              min={1}
              max={65535}
              onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                const value = Number(e.target.value);
                if (!Number.isNaN(value)) {
                  updateBackendSetting('port', value);
                }
              }}
            />
            <PasswordInput
              label={t('settings.remoteAccessToken', { defaultValue: 'Access token (optional)' })}
              value={profileSettings?.backend?.token ?? ''}
              onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                updateBackendSetting('token', e.target.value)
              }
              helper={t('settings.remoteAccessTokenHelper', {
                defaultValue:
                  'Clients must send this token as a Bearer auth header when enabled.',
              })}
              autoComplete="off"
              renderToggleIcon={(visible) =>
                visible ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />
              }
            />
          </div>
        </div>
      </CardBody>
    </Card>
  );
}
