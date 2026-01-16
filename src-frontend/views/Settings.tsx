import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderOpen, Download, Trash2, Github, BookOpen, RefreshCw } from 'lucide-react';
// import { open } from '@tauri-apps/api/shell';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Select } from '@/components/ui/Select';
import { Input } from '@/components/ui/Input';
import { Toggle } from '@/components/ui/Toggle';
import { Grid } from '@/components/ui/Grid';
import { Modal } from '@/components/ui/Modal';
import { Logo } from '@/components/layout/Logo';
import { FFmpegDownloadProgress } from '@/components/settings/FFmpegDownloadProgress';
import { KeyRotationSection } from '@/components/settings/KeyRotationSection';
import { api } from '@/lib/tauri';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { open as openPath } from '@tauri-apps/plugin-shell';
import { useLanguageStore, type Language } from '@/stores/languageStore';
import { useThemeStore } from '@/stores/themeStore';
import { useProfileStore } from '@/stores/profileStore';
import type { AppSettings } from '@/types/api';

interface SettingsState {
  // General
  language: string;
  startMinimized: boolean;
  showNotifications: boolean;
  // FFmpeg
  ffmpegPath: string;
  ffmpegVersion: string;
  autoDownloadFfmpeg: boolean;
  // Data & Privacy
  profileStoragePath: string;
  encryptStreamKeys: boolean;
  logRetentionDays: number;
  // UI state
  loading: boolean;
  saving: boolean;
}

const defaultSettings: SettingsState = {
  language: 'en',
  startMinimized: false,
  showNotifications: true,
  ffmpegPath: '',
  ffmpegVersion: '', // Will be translated when displayed
  autoDownloadFfmpeg: true,
  profileStoragePath: '',
  encryptStreamKeys: false,
  logRetentionDays: 30,
  loading: true,
  saving: false,
};

export function Settings() {
  const { t } = useTranslation();
  const { setLanguage, initFromSettings } = useLanguageStore();
  const { currentThemeId, themes, setTheme, refreshThemes } = useThemeStore();
  const { current: currentProfile, updateProfile } = useProfileStore();
  const [settings, setSettings] = useState<SettingsState>(defaultSettings);
  const [clearConfirmOpen, setClearConfirmOpen] = useState(false);
  const [clearInProgress, setClearInProgress] = useState(false);
  const [clearError, setClearError] = useState<string | null>(null);
  const [themeInstallError, setThemeInstallError] = useState<string | null>(null);
  const [themeInstalling, setThemeInstalling] = useState(false);

  // Load settings on mount
  useEffect(() => {
    const loadSettings = async () => {
      try {
        setSettings((prev) => ({ ...prev, loading: true }));

        // Load global settings from backend (used as fallback)
        const backendSettings = await api.settings.get();
        const profilesPath = await api.settings.getProfilesPath();

        // Get FFmpeg version and path
        let ffmpegVersion = ''; // Empty means not found (will be translated when displayed)
        let detectedFfmpegPath = '';
        try {
          ffmpegVersion = await api.system.testFfmpeg();
          // Get the detected path (either custom from settings or system install location)
          const path = await api.system.getFfmpegPath();
          if (path) {
            detectedFfmpegPath = path;
          }
        } catch {
          // FFmpeg not available
        }

        // Profile-specific settings (with fallback to global)
        const effectiveLanguage = currentProfile?.language || backendSettings.language;
        const effectiveTheme = currentProfile?.theme || backendSettings.themeId;

        // Initialize i18n with the effective language
        initFromSettings(effectiveLanguage);

        // Apply the effective theme
        if (effectiveTheme !== currentThemeId) {
          setTheme(effectiveTheme);
        }

        // Load chat configs from profile
        const twitchConfig = currentProfile?.chatConfigs?.find((c) => c.platform === 'twitch');
        const tiktokConfig = currentProfile?.chatConfigs?.find((c) => c.platform === 'tiktok');

        // Debug logging to track what's being loaded
        console.log('[Settings] Loading chat configs from profile:', {
          profileId: currentProfile?.id,
          profileName: currentProfile?.name,
          totalChatConfigs: currentProfile?.chatConfigs?.length || 0,
          twitchConfig: twitchConfig ? {
            platform: twitchConfig.platform,
            enabled: twitchConfig.enabled,
            hasChannel: twitchConfig.credentials.type === 'twitch' && !!twitchConfig.credentials.channel,
            hasToken: twitchConfig.credentials.type === 'twitch' && !!twitchConfig.credentials.oauthToken,
          } : null,
        });

        // Use detected path if available, otherwise fall back to saved settings path
        const ffmpegPath = detectedFfmpegPath || backendSettings.ffmpegPath;

        setSettings({
          language: effectiveLanguage,
          startMinimized: backendSettings.startMinimized,
          showNotifications: backendSettings.showNotifications,
          ffmpegPath,
          autoDownloadFfmpeg: backendSettings.autoDownloadFfmpeg,
          encryptStreamKeys: backendSettings.encryptStreamKeys,
          logRetentionDays: backendSettings.logRetentionDays,
          ffmpegVersion,
          profileStoragePath: profilesPath,
          twitchChannel: twitchConfig?.credentials.type === 'twitch' ? twitchConfig.credentials.channel : '',
          twitchOAuthToken: twitchConfig?.credentials.type === 'twitch' ? twitchConfig.credentials.oauthToken || '' : '',
          twitchStatus: twitchConfig?.enabled ? 'connected' : 'disconnected',
          twitchConnecting: false,
          tiktokUsername: tiktokConfig?.credentials.type === 'tiktok' ? tiktokConfig.credentials.username : '',
          tiktokSessionToken: tiktokConfig?.credentials.type === 'tiktok' ? tiktokConfig.credentials.sessionToken || '' : '',
          tiktokStatus: tiktokConfig?.enabled ? 'connected' : 'disconnected',
          tiktokConnecting: false,
          loading: false,
          saving: false,
        });
      } catch (error) {
        console.error('Failed to load settings:', error);
        setSettings((prev) => ({ ...prev, loading: false }));
      }
    };

    loadSettings();
    // initFromSettings is stable (from Zustand store), intentionally excluded
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentProfile]);

  useEffect(() => {
    refreshThemes().catch(() => {
      // Errors are already logged in the store
    });
  }, [refreshThemes]);

  // Save settings to backend
  const saveSettings = useCallback(
    async (newSettings: Partial<SettingsState>) => {
      const updatedSettings = { ...settings, ...newSettings };
      setSettings((prev) => ({ ...prev, ...newSettings, saving: true }));

      try {
        const backendSettings: AppSettings = {
          language: updatedSettings.language,
          startMinimized: updatedSettings.startMinimized,
          showNotifications: updatedSettings.showNotifications,
          ffmpegPath: updatedSettings.ffmpegPath,
          autoDownloadFfmpeg: updatedSettings.autoDownloadFfmpeg,
          encryptStreamKeys: updatedSettings.encryptStreamKeys,
          logRetentionDays: updatedSettings.logRetentionDays,
          themeId: useThemeStore.getState().currentThemeId,
          lastProfile: null, // Preserve existing or let backend handle
        };

        await api.settings.save(backendSettings);
      } catch (error) {
        console.error('Failed to save settings:', error);
      } finally {
        setSettings((prev) => ({ ...prev, saving: false }));
      }
    },
    [settings]
  );

  const updateSetting = <K extends keyof SettingsState>(key: K, value: SettingsState[K]) => {
    // Handle profile-specific settings
    if (key === 'language') {
      const lang = value as string;
      setLanguage(lang as Language);
      // Save to profile if one is active
      if (currentProfile) {
        updateProfile({ language: lang }).catch((err) => console.error('Failed to update profile language:', err));
      }
      // Also save to global settings as fallback
      saveSettings({ language: lang });
    } else {
      // For non-profile settings, save to global
      saveSettings({ [key]: value });
    }
  };

  const handleBrowseFfmpeg = async () => {
    try {
      const selected = await openDialog({
        multiple: false,
        filters: [{ name: 'FFmpeg', extensions: ['*'] }],
      });
      if (selected && typeof selected === 'string') {
        // Validate the selected path before saving
        try {
          const version = await api.system.validateFfmpegPath(selected);
          // Path is valid, save it and update version
          saveSettings({ ffmpegPath: selected });
          setSettings((prev) => ({ ...prev, ffmpegVersion: version }));
        } catch (validationError) {
          // Show error to user
          alert(`${t('settings.invalidFfmpegPath')}: ${validationError}`);
        }
      }
    } catch (error) {
      console.error('Failed to open file dialog:', error);
    }
  };

  const handleOpenProfileStorage = async () => {
    try {
      await openPath(settings.profileStoragePath);
    } catch (error) {
      console.error('Failed to open profile storage:', error);
    }
  };

  const handleExportData = async () => {
    try {
      const selected = await openDialog({
        directory: true,
        multiple: false,
        title: t('settings.selectExportLocation'),
      });
      if (selected && typeof selected === 'string') {
        await api.settings.exportData(selected);
        alert(t('toast.dataExported'));
      }
    } catch (error) {
      console.error('Failed to export data:', error);
      alert(`${t('settings.exportFailed')}: ${error}`);
    }
  };

  const handleInstallTheme = async () => {
    setThemeInstallError(null);
    setThemeInstalling(true);
    try {
      const selected = await openDialog({
        multiple: false,
        filters: [{ name: 'Theme', extensions: ['json', 'jsonc'] }],
        title: t('settings.installTheme', { defaultValue: 'Install Theme' }),
      });
      if (selected && typeof selected === 'string') {
        await api.theme.install(selected);
        await refreshThemes();
      }
    } catch (error) {
      console.error('Failed to install theme:', error);
      setThemeInstallError(String(error));
    } finally {
      setThemeInstalling(false);
    }
  };

  // Chat connection handlers
  const handleTwitchConnect = async () => {
    if (!settings.twitchChannel.trim()) {
      alert(t('chat.config.errors.invalidChannel'));
      return;
    }

    if (!currentProfile) {
      alert(t('chat.config.errors.noActiveProfile', { defaultValue: 'No active profile. Please load or create a profile first.' }));
      return;
    }

    setSettings((prev) => ({ ...prev, twitchConnecting: true }));
    try {
      const config: ChatConfig = {
        platform: 'twitch' as ChatPlatform,
        enabled: true,
        credentials: {
          type: 'twitch',
          channel: settings.twitchChannel.trim(),
          oauthToken: settings.twitchOAuthToken.trim() || undefined,
        },
      };
      await api.chat.connect(config);
      setSettings((prev) => ({ ...prev, twitchStatus: 'connected' }));

      // Save chat config to profile
      const existingConfigs = currentProfile.chatConfigs || [];
      const updatedConfigs = existingConfigs.filter((c) => c.platform !== 'twitch');
      updatedConfigs.push(config);
      console.log('[Settings] Saving Twitch config to profile:', {
        profileId: currentProfile.id,
        profileName: currentProfile.name,
        configCount: updatedConfigs.length,
        twitchChannel: config.credentials.type === 'twitch' ? config.credentials.channel : null,
      });
      await updateProfile({ chatConfigs: updatedConfigs });
    } catch (error) {
      console.error('Failed to connect to Twitch:', error);
      alert(t('chat.config.errors.connectionFailed', { error: String(error) }));
      setSettings((prev) => ({ ...prev, twitchStatus: 'error' }));
    } finally {
      setSettings((prev) => ({ ...prev, twitchConnecting: false }));
    }
  };

  const handleTwitchDisconnect = async () => {
    if (!currentProfile) return;

    setSettings((prev) => ({ ...prev, twitchConnecting: true }));
    try {
      await api.chat.disconnect('twitch' as ChatPlatform);
      setSettings((prev) => ({ ...prev, twitchStatus: 'disconnected' }));

      // Update chat config in profile to mark as disabled (but preserve credentials)
      const existingConfigs = currentProfile.chatConfigs || [];
      const updatedConfigs = existingConfigs.map((c) =>
        c.platform === 'twitch' ? { ...c, enabled: false } : c
      );
      console.log('[Settings] Disabling Twitch config (preserving credentials):', {
        profileId: currentProfile.id,
        profileName: currentProfile.name,
        configCount: updatedConfigs.length,
        twitchConfigExists: updatedConfigs.some((c) => c.platform === 'twitch'),
      });
      await updateProfile({ chatConfigs: updatedConfigs });
    } catch (error) {
      console.error('Failed to disconnect from Twitch:', error);
      alert(t('chat.config.errors.disconnectionFailed', { error: String(error) }));
    } finally {
      setSettings((prev) => ({ ...prev, twitchConnecting: false }));
    }
  };

  const handleTikTokConnect = async () => {
    if (!settings.tiktokUsername.trim()) {
      alert(t('chat.config.errors.invalidUsername'));
      return;
    }

    if (!currentProfile) {
      alert(t('chat.config.errors.noActiveProfile', { defaultValue: 'No active profile. Please load or create a profile first.' }));
      return;
    }

    setSettings((prev) => ({ ...prev, tiktokConnecting: true }));
    try {
      const config: ChatConfig = {
        platform: 'tiktok' as ChatPlatform,
        enabled: true,
        credentials: {
          type: 'tiktok',
          username: settings.tiktokUsername.trim(),
          sessionToken: settings.tiktokSessionToken.trim() || undefined,
        },
      };
      await api.chat.connect(config);
      setSettings((prev) => ({ ...prev, tiktokStatus: 'connected' }));

      // Save chat config to profile
      const existingConfigs = currentProfile.chatConfigs || [];
      const updatedConfigs = existingConfigs.filter((c) => c.platform !== 'tiktok');
      updatedConfigs.push(config);
      await updateProfile({ chatConfigs: updatedConfigs });
    } catch (error) {
      console.error('Failed to connect to TikTok:', error);
      alert(t('chat.config.errors.connectionFailed', { error: String(error) }));
      setSettings((prev) => ({ ...prev, tiktokStatus: 'error' }));
    } finally {
      setSettings((prev) => ({ ...prev, tiktokConnecting: false }));
    }
  };

  const handleTikTokDisconnect = async () => {
    if (!currentProfile) return;

    setSettings((prev) => ({ ...prev, tiktokConnecting: true }));
    try {
      await api.chat.disconnect('tiktok' as ChatPlatform);
      setSettings((prev) => ({ ...prev, tiktokStatus: 'disconnected' }));

      // Update chat config in profile to mark as disabled
      const existingConfigs = currentProfile.chatConfigs || [];
      const updatedConfigs = existingConfigs.map((c) =>
        c.platform === 'tiktok' ? { ...c, enabled: false } : c
      );
      await updateProfile({ chatConfigs: updatedConfigs });
    } catch (error) {
      console.error('Failed to disconnect from TikTok:', error);
      alert(t('chat.config.errors.disconnectionFailed', { error: String(error) }));
    } finally {
      setSettings((prev) => ({ ...prev, tiktokConnecting: false }));
    }
  };

  const handleClearAllData = () => {
    setClearError(null);
    setClearConfirmOpen(true);
  };

  const handleClearCancel = () => {
    if (clearInProgress) return;
    setClearConfirmOpen(false);
    setClearError(null);
  };

  const handleClearConfirm = async () => {
    setClearInProgress(true);
    setClearError(null);
    try {
      await api.settings.clearData();
      alert(t('toast.dataCleared'));
      // Reset to defaults
      setSettings({ ...defaultSettings, loading: false, saving: false });
      setClearConfirmOpen(false);
    } catch (error) {
      console.error('Failed to clear data:', error);
      setClearError(`${t('settings.clearFailed')}: ${error}`);
    } finally {
      setClearInProgress(false);
    }
  };

  const languageOptions = [
    { value: 'en', label: 'English' },
    { value: 'es', label: 'Espa\u00f1ol' },
    { value: 'fr', label: 'Fran\u00e7ais' },
    { value: 'de', label: 'Deutsch' },
    { value: 'ja', label: '\u65e5\u672c\u8a9e' },
    { value: 'ar', label: '\u0627\u0644\u0639\u0631\u0628\u064a\u0629' },
    { value: 'zh-CN', label: '\u4e2d\u6587(\u7b80\u4f53)' },
    { value: 'ko', label: '\ud55c\uad6d\uc5b4' },
    { value: 'uk', label: '\u0423\u043a\u0440\u0430\u0457\u043d\u0441\u044c\u043a\u0430' },
    { value: 'ru', label: '\u0420\u0443\u0441\u0441\u043a\u0438\u0439' },
    { value: 'af', label: 'Afrikaans' },
  ];

  const logRetentionOptions = [
    { value: 7, label: t('settings.logRetention7Days', { defaultValue: '7 days' }) },
    { value: 14, label: t('settings.logRetention14Days', { defaultValue: '14 days' }) },
    { value: 30, label: t('settings.logRetention30Days', { defaultValue: '30 days' }) },
    { value: 90, label: t('settings.logRetention90Days', { defaultValue: '90 days' }) },
    { value: 365, label: t('settings.logRetention365Days', { defaultValue: '365 days' }) },
  ];

  const themeOptions = (themes.length
    ? themes
    : [
        {
          id: 'spirit-light',
          name: 'Spirit Light',
          mode: 'light' as const,
          source: 'builtin' as const,
        },
        {
          id: 'spirit-dark',
          name: 'Spirit Dark',
          mode: 'dark' as const,
          source: 'builtin' as const,
        },
      ]
  ).map((themeItem) => ({
    value: themeItem.id,
    label: themeItem.name,
  }));

  const handleThemeChange = async (themeId: string) => {
    await setTheme(themeId);
    // Save to profile if one is active
    if (currentProfile) {
      await updateProfile({ theme: themeId }).catch((err) => console.error('Failed to update profile theme:', err));
    }
    // Also save to global settings as fallback
    saveSettings({});
  };

  if (settings.loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-[var(--text-secondary)]">{t('common.loading')}</div>
      </div>
    );
  }

  return (
    <Grid cols={2}>
      {/* General Settings */}
      <Card>
        <CardHeader>
          <div>
            <CardTitle>{t('settings.generalSettings')}</CardTitle>
            <CardDescription>{t('settings.generalDescription')}</CardDescription>
          </div>
        </CardHeader>
        <CardBody
          style={{ padding: '24px', display: 'flex', flexDirection: 'column', gap: '16px' }}
        >
          <Select
            label={t('settings.language')}
            value={settings.language}
            onChange={(e: React.ChangeEvent<HTMLSelectElement>) =>
              updateSetting('language', e.target.value)
            }
            options={languageOptions}
          />
          <div className="flex items-center justify-between" style={{ padding: '8px 0' }}>
            <div>
              <div className="text-sm font-medium text-[var(--text-primary)]">
                {t('settings.startMinimized')}
              </div>
              <div className="text-xs text-[var(--text-tertiary)]">
                {t('settings.startMinimizedDescription')}
              </div>
            </div>
            <Toggle
              checked={settings.startMinimized}
              onChange={(checked: boolean) => updateSetting('startMinimized', checked)}
            />
          </div>
          <div className="flex items-center justify-between" style={{ padding: '8px 0' }}>
            <div>
              <div className="text-sm font-medium text-[var(--text-primary)]">
                {t('settings.showNotifications')}
              </div>
              <div className="text-xs text-[var(--text-tertiary)]">
                {t('settings.showNotificationsDescription')}
              </div>
            </div>
            <Toggle
              checked={settings.showNotifications}
              onChange={(checked: boolean) => updateSetting('showNotifications', checked)}
            />
          </div>
        </CardBody>
      </Card>

      {/* Chat Configuration */}
      <Card>
        <CardHeader>
          <div>
            <CardTitle>
              <div className="flex items-center gap-2">
                <MessageCircle className="w-5 h-5" />
                {t('chat.config.title')}
              </div>
            </CardTitle>
            <CardDescription>{t('chat.config.description')}</CardDescription>
          </div>
        </CardHeader>
        <CardBody
          style={{ padding: '24px', display: 'flex', flexDirection: 'column', gap: '24px' }}
        >
          {/* Twitch Configuration */}
          <div className="flex flex-col" style={{ gap: '16px' }}>
            <div className="flex items-center justify-between">
              <div>
                <div className="text-sm font-semibold text-[var(--text-primary)]">
                  {t('chat.config.twitch.title')}
                </div>
                <div className="text-xs text-[var(--text-tertiary)]">
                  {t('chat.config.twitch.description')}
                </div>
              </div>
              {settings.twitchStatus === 'connected' && (
                <div className="flex items-center gap-2">
                  <div className="w-2 h-2 rounded-full bg-[var(--status-live)] animate-pulse" />
                  <span className="text-xs font-medium text-[var(--status-live-text)]">
                    {t('chat.config.status.connected')}
                  </span>
                </div>
              )}
            </div>
            <Input
              label={t('chat.config.twitch.channel')}
              placeholder={t('chat.config.twitch.channelPlaceholder')}
              value={settings.twitchChannel}
              onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                setSettings((prev) => ({ ...prev, twitchChannel: e.target.value }))
              }
              helper={
                settings.twitchStatus === 'disconnected' && settings.twitchChannel
                  ? t('chat.config.credentialsSaved', { defaultValue: '✓ Credentials saved in this profile' })
                  : t('chat.config.twitch.channelHelper')
              }
              disabled={settings.twitchStatus === 'connected'}
            />
            <Input
              label={t('chat.config.twitch.oauthToken')}
              type="password"
              placeholder={t('chat.config.twitch.oauthTokenPlaceholder')}
              value={settings.twitchOAuthToken}
              onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                setSettings((prev) => ({ ...prev, twitchOAuthToken: e.target.value }))
              }
              helper={t('chat.config.twitch.oauthTokenHelper')}
              disabled={settings.twitchStatus === 'connected'}
            />
            <div>
              {settings.twitchStatus === 'connected' ? (
                <Button
                  variant="outline"
                  onClick={handleTwitchDisconnect}
                  disabled={settings.twitchConnecting}
                >
                  {settings.twitchConnecting
                    ? t('chat.config.disconnecting')
                    : t('chat.config.disconnect')}
                </Button>
              ) : (
                <Button
                  variant="primary"
                  onClick={handleTwitchConnect}
                  disabled={settings.twitchConnecting || !settings.twitchChannel.trim()}
                >
                  {settings.twitchConnecting
                    ? t('chat.config.connecting')
                    : t('chat.config.connect')}
                </Button>
              )}
            </div>
          </div>

          {/* Divider */}
          <div className="border-t border-[var(--border-muted)]" />

          {/* TikTok Configuration */}
          <div className="flex flex-col" style={{ gap: '16px' }}>
            <div className="flex items-center justify-between">
              <div className="flex-1">
                <div className="flex items-center gap-2">
                  <div className="text-sm font-semibold text-[var(--text-primary)]">
                    {t('chat.config.tiktok.title')}
                  </div>
                  <div className="px-2 py-0.5 text-xs font-medium bg-[var(--warning-subtle)] text-[var(--warning-text)] rounded-full">
                    {t('chat.config.tiktok.experimental')}
                  </div>
                </div>
                <div className="text-xs text-[var(--text-tertiary)]">
                  {t('chat.config.tiktok.description')}
                </div>
              </div>
              {settings.tiktokStatus === 'connected' && (
                <div className="flex items-center gap-2">
                  <div className="w-2 h-2 rounded-full bg-[var(--status-live)] animate-pulse" />
                  <span className="text-xs font-medium text-[var(--status-live-text)]">
                    {t('chat.config.status.connected')}
                  </span>
                </div>
              )}
            </div>

            {/* Warning about experimental status */}
            <div className="flex items-start gap-2 p-3 rounded-lg bg-[var(--warning-subtle)] border border-[var(--warning-border)]">
              <AlertTriangle className="w-4 h-4 text-[var(--warning-text)] flex-shrink-0 mt-0.5" />
              <div className="flex-1">
                <p className="text-xs text-[var(--warning-text)]">
                  {t('chat.config.tiktok.notImplementedWarning')}
                </p>
              </div>
            </div>

            <Input
              label={t('chat.config.tiktok.username')}
              placeholder={t('chat.config.tiktok.usernamePlaceholder')}
              value={settings.tiktokUsername}
              onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                setSettings((prev) => ({ ...prev, tiktokUsername: e.target.value }))
              }
              helper={t('chat.config.tiktok.usernameHelper')}
              disabled={settings.tiktokStatus === 'connected'}
            />
            <Input
              label={t('chat.config.tiktok.sessionToken')}
              type="password"
              placeholder={t('chat.config.tiktok.sessionTokenPlaceholder')}
              value={settings.tiktokSessionToken}
              onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                setSettings((prev) => ({ ...prev, tiktokSessionToken: e.target.value }))
              }
              helper={t('chat.config.tiktok.sessionTokenHelper')}
              disabled={settings.tiktokStatus === 'connected'}
            />
            <div>
              {settings.tiktokStatus === 'connected' ? (
                <Button
                  variant="outline"
                  onClick={handleTikTokDisconnect}
                  disabled={settings.tiktokConnecting}
                >
                  {settings.tiktokConnecting
                    ? t('chat.config.disconnecting')
                    : t('chat.config.disconnect')}
                </Button>
              ) : (
                <Button
                  variant="primary"
                  onClick={handleTikTokConnect}
                  disabled={settings.tiktokConnecting || !settings.tiktokUsername.trim()}
                >
                  {settings.tiktokConnecting
                    ? t('chat.config.connecting')
                    : t('chat.config.connect')}
                </Button>
              )}
            </div>
          </div>
        </CardBody>
      </Card>

      {/* FFmpeg Configuration */}
      <Card>
        <CardHeader>
          <div>
            <CardTitle>{t('settings.ffmpegConfig')}</CardTitle>
            <CardDescription>{t('settings.ffmpegDescription')}</CardDescription>
          </div>
        </CardHeader>
        <CardBody
          style={{ padding: '24px', display: 'flex', flexDirection: 'column', gap: '16px' }}
        >
          <div className="flex flex-col" style={{ gap: '6px' }}>
            <label className="block text-sm font-medium text-[var(--text-primary)]">
              {t('settings.ffmpegPath')}
            </label>
            <div className="flex" style={{ gap: '8px' }}>
              <Input
                value={settings.ffmpegPath}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                  updateSetting('ffmpegPath', e.target.value)
                }
                className="flex-1"
              />
              <Button variant="outline" onClick={handleBrowseFfmpeg}>
                <FolderOpen className="w-4 h-4" />
                {t('settings.browse')}
              </Button>
            </div>
          </div>
          <Input
            label={t('settings.ffmpegVersion')}
            value={
              settings.loading
                ? t('settings.detecting')
                : settings.ffmpegVersion || t('settings.ffmpegNotFound')
            }
            disabled
            helper={t('settings.detectedVersion')}
          />

          {/* FFmpeg Status Section */}
          <div className="border-t border-[var(--border-muted)]" style={{ paddingTop: '16px' }}>
            <FFmpegDownloadProgress
              installedVersion={settings.ffmpegVersion || undefined}
              onComplete={(path: string) => {
                // Immediately update local state for live feedback
                setSettings((prev) => ({ ...prev, ffmpegPath: path }));
                // Save to backend
                saveSettings({ ffmpegPath: path });
                // Refresh FFmpeg version
                api.system
                  .testFfmpeg()
                  .then((version: string) => {
                    setSettings((prev) => ({ ...prev, ffmpegVersion: version }));
                  })
                  .catch(() => {
                    // Ignore errors
                  });
              }}
            />
          </div>
        </CardBody>
      </Card>

      {/* Themes */}
      <Card>
        <CardHeader>
          <div>
            <CardTitle>{t('settings.themes', { defaultValue: 'Themes' })}</CardTitle>
            <CardDescription>
              {t('settings.themesDescription', {
                defaultValue: 'Choose a UI theme and install custom themes.',
              })}
            </CardDescription>
          </div>
        </CardHeader>
        <CardBody
          style={{ padding: '24px', display: 'flex', flexDirection: 'column', gap: '16px' }}
        >
          <Select
            label={t('settings.theme', { defaultValue: 'Theme' })}
            value={currentThemeId}
            onChange={(e: React.ChangeEvent<HTMLSelectElement>) => handleThemeChange(e.target.value)}
            options={themeOptions}
            helper={t('settings.themeHelper', {
              defaultValue: 'Choose your preferred theme appearance.',
            })}
          />
          <div className="flex items-center" style={{ gap: '12px' }}>
            <Button variant="outline" onClick={handleInstallTheme} disabled={themeInstalling}>
              {themeInstalling
                ? t('common.loading')
                : t('settings.installTheme', { defaultValue: 'Install Theme' })}
            </Button>
          </div>
          {themeInstallError && (
            <div className="p-3 rounded-lg bg-[var(--error-subtle)] border border-[var(--error-border)]">
              <p className="text-sm text-[var(--error-text)]">{themeInstallError}</p>
            </div>
          )}
        </CardBody>
      </Card>

      {/* Data & Privacy */}
      <Card>
        <CardHeader>
          <div>
            <CardTitle>{t('settings.dataPrivacy')}</CardTitle>
            <CardDescription>{t('settings.dataPrivacyDescription')}</CardDescription>
          </div>
        </CardHeader>
        <CardBody
          style={{ padding: '24px', display: 'flex', flexDirection: 'column', gap: '16px' }}
        >
          <div className="flex flex-col" style={{ gap: '6px' }}>
            <label className="block text-sm font-medium text-[var(--text-primary)]">
              {t('settings.profileStorage')}
            </label>
            <div className="flex" style={{ gap: '8px' }}>
              <Input
                value={settings.profileStoragePath}
                disabled
                className="flex-1 font-mono text-xs"
              />
              <Button variant="outline" onClick={handleOpenProfileStorage}>
                <FolderOpen className="w-4 h-4" />
                {t('settings.open')}
              </Button>
            </div>
          </div>
          <div className="flex items-center justify-between" style={{ padding: '8px 0' }}>
            <div>
              <div className="text-sm font-medium text-[var(--text-primary)]">
                {t('settings.encryptStreamKeys')}
              </div>
              <div className="text-xs text-[var(--text-tertiary)]">
                {t('settings.encryptStreamKeysDescription')}
              </div>
            </div>
            <Toggle
              checked={settings.encryptStreamKeys}
              onChange={(checked: boolean) => updateSetting('encryptStreamKeys', checked)}
            />
          </div>
          <KeyRotationSection
            encryptStreamKeys={settings.encryptStreamKeys}
            disabled={settings.saving}
          />
          <Select
            label={t('settings.logRetention', { defaultValue: 'Log retention' })}
            value={String(settings.logRetentionDays)}
            onChange={(e) => updateSetting('logRetentionDays', Number(e.target.value))}
            options={logRetentionOptions.map((option) => ({
              value: String(option.value),
              label: option.label,
            }))}
            helper={t('settings.logRetentionDescription', {
              defaultValue: 'How long to keep application log files.',
            })}
          />
          <div
            className="border-t border-[var(--border-muted)] flex"
            style={{ paddingTop: '16px', gap: '12px' }}
          >
            <Button variant="outline" onClick={handleExportData}>
              <Download className="w-4 h-4" />
              {t('settings.exportData')}
            </Button>
            <Button variant="destructive" onClick={handleClearAllData}>
              <Trash2 className="w-4 h-4" />
              {t('settings.clearAllData')}
            </Button>
          </div>
        </CardBody>
      </Card>

      {/* About */}
      <Card>
        <CardHeader>
          <div>
            <CardTitle>{t('settings.about')}</CardTitle>
            <CardDescription>{t('settings.aboutDescription')}</CardDescription>
          </div>
        </CardHeader>
        <CardBody>
          <div className="text-center" style={{ padding: '16px 0' }}>
            <div className="flex justify-center" style={{ marginBottom: '16px' }}>
              <Logo size="lg" />
            </div>
            <div className="text-sm text-[var(--text-secondary)]" style={{ marginBottom: '4px' }}>
              {t('settings.version')} 0.1.0
            </div>
            <div className="text-xs text-[var(--text-tertiary)]" style={{ marginBottom: '24px' }}>
              {t('settings.tagline')}
            </div>
            <div className="flex justify-center" style={{ gap: '12px' }}>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => openPath('https://github.com/ScopeCreep-zip/SpiritStream')}
              >
                <Github className="w-4 h-4" />
                {t('settings.github')}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => openPath('https://deepwiki.com/ScopeCreep-zip/SpiritStream')}
              >
                <BookOpen className="w-4 h-4" />
                {t('settings.docs')}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() =>
                  window.open('https://github.com/ScopeCreep-zip/SpiritStream/releases', '_blank')
                }
              >
                <RefreshCw className="w-4 h-4" />
                {t('settings.updates')}
              </Button>
            </div>
          </div>
        </CardBody>
      </Card>
      <Modal
        open={clearConfirmOpen}
        onClose={handleClearCancel}
        title={t('settings.clearAllData')}
        footer={
          <>
            <Button variant="ghost" onClick={handleClearCancel} disabled={clearInProgress}>
              {t('common.cancel')}
            </Button>
            <Button variant="destructive" onClick={handleClearConfirm} disabled={clearInProgress}>
              {clearInProgress ? t('common.loading') : t('common.confirm')}
            </Button>
          </>
        }
      >
        <p className="text-[var(--text-secondary)]">{t('settings.clearConfirm')}</p>
        {clearError && (
          <div className="mt-4 p-3 rounded-lg bg-[var(--error-subtle)] border border-[var(--error-border)]">
            <p className="text-sm text-[var(--error-text)]">{clearError}</p>
          </div>
        )}
      </Modal>
    </Grid>
  );
}
