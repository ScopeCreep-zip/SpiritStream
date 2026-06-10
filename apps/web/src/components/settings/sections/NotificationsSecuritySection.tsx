import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Toggle } from '@/components/ui/Toggle';
import { useProfileStore } from '@/stores/profileStore';
import { useSaveSettings, useUpdateSetting } from '@/hooks/useSettings';
import { KeyRotationSection } from '@/components/settings/KeyRotationSection';
import type { ProfileSettings as ProfileSettingsType } from '@spiritstream/types';

/**
 * Notification + at-rest-encryption toggles for the active profile, plus
 * the bundled key-rotation flow.
 */
export function NotificationsSecuritySection() {
  const { t } = useTranslation();
  const currentProfile = useProfileStore((state) => state.current);
  const updateProfileSettings = useProfileStore((state) => state.updateProfileSettings);
  const updateSettingMutation = useUpdateSetting();
  const saveSettingsMutation = useSaveSettings();
  const profileSettings = currentProfile?.settings;

  const isSaving = updateSettingMutation.isPending || saveSettingsMutation.isPending;

  const updateProfileSetting = useCallback(
    async <K extends keyof ProfileSettingsType>(key: K, value: ProfileSettingsType[K]) => {
      if (!profileSettings) return;
      await updateProfileSettings({ [key]: value });
    },
    [profileSettings, updateProfileSettings]
  );

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle>
            {t('settings.notificationsSecurity', {
              defaultValue: 'Notifications & Security',
            })}
          </CardTitle>
          <CardDescription>
            {t('settings.notificationsSecurityDescription', {
              defaultValue: 'Notification preferences and data security for this profile.',
            })}
          </CardDescription>
        </div>
      </CardHeader>
      <CardBody className="p-6 flex flex-col gap-4">
        <div className="flex items-center justify-between py-2">
          <div>
            <div className="text-sm font-medium text-text-primary">
              {t('settings.showNotifications')}
            </div>
            <div className="text-xs text-text-tertiary">
              {t('settings.showNotificationsDescription')}
            </div>
          </div>
          <Toggle
            checked={profileSettings?.showNotifications ?? true}
            onChange={(checked: boolean) => updateProfileSetting('showNotifications', checked)}
          />
        </div>
        <div className="flex items-center justify-between py-2">
          <div>
            <div className="text-sm font-medium text-text-primary">
              {t('settings.encryptStreamKeys')}
            </div>
            <div className="text-xs text-text-tertiary">
              {t('settings.encryptStreamKeysDescription')}
            </div>
          </div>
          <Toggle
            checked={profileSettings?.encryptStreamKeys ?? true}
            onChange={(checked: boolean) => updateProfileSetting('encryptStreamKeys', checked)}
          />
        </div>
        <KeyRotationSection
          encryptStreamKeys={profileSettings?.encryptStreamKeys ?? true}
          disabled={isSaving}
        />
      </CardBody>
    </Card>
  );
}
