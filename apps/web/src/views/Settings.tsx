import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Globe, User } from 'lucide-react';
import { Grid } from '@/components/ui/Grid';
import { useSettings, useSettingsSync } from '@/hooks/useSettings';
import { useProfileStore } from '@/stores/profileStore';
import { AppearanceSection } from '@/components/settings/sections/AppearanceSection';
import { AccessibilitySection } from '@/components/settings/sections/AccessibilitySection';
import { NotificationsSecuritySection } from '@/components/settings/sections/NotificationsSecuritySection';
import { RemoteAccessSection } from '@/components/settings/sections/RemoteAccessSection';
import { FFmpegConfigSection } from '@/components/settings/sections/FFmpegConfigSection';
import { AppBehaviorSection } from '@/components/settings/sections/AppBehaviorSection';
import { DataManagementSection } from '@/components/settings/sections/DataManagementSection';
import { AboutSection } from '@/components/settings/sections/AboutSection';
import { cn } from '@/lib/cn';

type SettingsTab = 'global' | 'profile';

/**
 * Settings — top-level tab coordinator. Each visible card is its own
 * self-contained component under `components/settings/sections/`; this
 * file only owns the Profile / Global tab switch and global loading /
 * error states. Per `feedback_preserve_menus`, the Profile / Global tab
 * IA is intentionally preserved.
 */
export function Settings() {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<SettingsTab>('profile');
  const { data: settings, isLoading, isError } = useSettings();
  const currentProfile = useProfileStore((state) => state.current);

  // Sync with remote changes.
  useSettingsSync();

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-text-secondary">{t('common.loading')}</div>
      </div>
    );
  }

  if (isError || !settings) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-error-text">
          {t('settings.loadError', { defaultValue: 'Failed to load settings' })}
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex gap-2 border-b border-border-default pb-2">
        <button
          onClick={() => setActiveTab('profile')}
          className={cn(
            'flex items-center gap-2 px-4 py-2 rounded-t-lg text-sm font-medium transition-colors',
            activeTab === 'profile'
              ? 'bg-bg-surface text-text-primary border border-b-0 border-border-default'
              : 'text-text-secondary hover:text-text-primary hover:bg-bg-muted',
          )}
        >
          <User className="w-4 h-4" />
          {t('settings.profileSettings', { defaultValue: 'Profile Settings' })}
          {currentProfile && (
            <span className="text-xs text-text-tertiary">({currentProfile.name})</span>
          )}
        </button>
        <button
          onClick={() => setActiveTab('global')}
          className={cn(
            'flex items-center gap-2 px-4 py-2 rounded-t-lg text-sm font-medium transition-colors',
            activeTab === 'global'
              ? 'bg-bg-surface text-text-primary border border-b-0 border-border-default'
              : 'text-text-secondary hover:text-text-primary hover:bg-bg-muted',
          )}
        >
          <Globe className="w-4 h-4" />
          {t('settings.globalSettings', { defaultValue: 'Global Settings' })}
        </button>
      </div>

      {activeTab === 'profile' && (
        <>
          {!currentProfile ? (
            <div className="flex items-center justify-center h-64 text-text-tertiary">
              {t('settings.loadProfileFirst', {
                defaultValue: 'Please load a profile to edit its settings.',
              })}
            </div>
          ) : (
            <Grid cols={2}>
              <AppearanceSection />
              <AccessibilitySection />
              <NotificationsSecuritySection />
              <RemoteAccessSection />
            </Grid>
          )}
        </>
      )}

      {activeTab === 'global' && (
        <Grid cols={2}>
          <FFmpegConfigSection />
          <AppBehaviorSection />
          <DataManagementSection />
          <AboutSection />
        </Grid>
      )}
    </div>
  );
}
