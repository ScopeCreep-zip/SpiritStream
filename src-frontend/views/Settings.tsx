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
import { api, dialogs } from '@/lib/backend';
import { useLanguageStore, type Language } from '@/stores/languageStore';
import { useThemeStore } from '@/stores/themeStore';
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
  // Remote access
  backendRemoteEnabled: boolean;
  backendUiEnabled: boolean;
  backendHost: string;
  backendPort: number;
  backendToken: string;
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
  backendRemoteEnabled: false,
  backendUiEnabled: false,
  backendHost: '127.0.0.1',
  backendPort: 8008,
  backendToken: '',
  loading: true,
  saving: false,
};

export function Settings() {
  const { t } = useTranslation();
  const { setLanguage, initFromSettings } = useLanguageStore();
  const { currentThemeId, themes, setTheme, refreshThemes } = useThemeStore();
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

        // Load settings from backend
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

        // Initialize i18n with the saved language
        initFromSettings(backendSettings.language);

        // Use detected path if available, otherwise fall back to saved settings path
        const ffmpegPath = detectedFfmpegPath || backendSettings.ffmpegPath;

        setSettings({
          language: backendSettings.language,
          startMinimized: backendSettings.startMinimized,
          showNotifications: backendSettings.showNotifications,
          ffmpegPath,
          autoDownloadFfmpeg: backendSettings.autoDownloadFfmpeg,
          encryptStreamKeys: backendSettings.encryptStreamKeys,
          logRetentionDays: backendSettings.logRetentionDays,
          backendRemoteEnabled: backendSettings.backendRemoteEnabled,
          backendUiEnabled: backendSettings.backendUiEnabled,
          backendHost: backendSettings.backendHost,
          backendPort: backendSettings.backendPort,
          backendToken: backendSettings.backendToken,
          ffmpegVersion,
          profileStoragePath: profilesPath,
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
  }, []);

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
          backendRemoteEnabled: updatedSettings.backendRemoteEnabled,
          backendUiEnabled: updatedSettings.backendUiEnabled,
          backendHost: updatedSettings.backendHost,
          backendPort: updatedSettings.backendPort,
          backendToken: updatedSettings.backendToken,
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
    // If changing language, update i18n as well
    if (key === 'language') {
      setLanguage(value as Language);
    }
    saveSettings({ [key]: value });
  };

  const handleBrowseFfmpeg = async () => {
    try {
      const selected = await dialogs.openFilePath({
        multiple: false,
        filters: [{ name: 'FFmpeg', extensions: ['*'] }],
      });
      if (!selected) {
        alert(
          t('settings.filePickerUnavailable', {
            defaultValue: 'File picker is not available in this environment.',
          })
        );
        return;
      }
      if (typeof selected === 'string') {
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
      await dialogs.openExternal(settings.profileStoragePath);
    } catch (error) {
      console.error('Failed to open profile storage:', error);
    }
  };

  const handleExportData = async () => {
    try {
      const selected = await dialogs.openDirectoryPath({
        multiple: false,
        title: t('settings.selectExportLocation'),
      });
      if (!selected) {
        alert(
          t('settings.exportUnavailable', {
            defaultValue: 'Export location selection is not available in this environment.',
          })
        );
        return;
      }
      await api.settings.exportData(selected);
      alert(t('toast.dataExported'));
    } catch (error) {
      console.error('Failed to export data:', error);
      alert(`${t('settings.exportFailed')}: ${error}`);
    }
  };

  const handleInstallTheme = async () => {
    setThemeInstallError(null);
    setThemeInstalling(true);
    try {
      const selected = await dialogs.openFilePath({
        multiple: false,
        filters: [{ name: 'Theme', extensions: ['json', 'jsonc'] }],
        title: t('settings.installTheme', { defaultValue: 'Install Theme' }),
      });
      if (!selected) {
        setThemeInstalling(false);
        alert(
          t('settings.filePickerUnavailable', {
            defaultValue: 'File picker is not available in this environment.',
          })
        );
        return;
      }
      await api.theme.install(selected);
      await refreshThemes();
    } catch (error) {
      console.error('Failed to install theme:', error);
      setThemeInstallError(String(error));
    } finally {
      setThemeInstalling(false);
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

      {/* Remote Access */}
      <Card>
        <CardHeader>
          <div>
            <CardTitle>{t('settings.remoteAccess', { defaultValue: 'Remote access' })}</CardTitle>
            <CardDescription>
              {t('settings.remoteAccessDescription', {
                defaultValue:
                  'Enable the built-in HTTP API so you can manage SpiritStream from another device.',
              })}
            </CardDescription>
          </div>
        </CardHeader>
        <CardBody
          style={{ padding: '24px', display: 'flex', flexDirection: 'column', gap: '16px' }}
        >
          <div className="flex items-center justify-between" style={{ padding: '8px 0' }}>
            <div>
              <div className="text-sm font-medium text-[var(--text-primary)]">
                {t('settings.remoteAccessToggle', {
                  defaultValue: 'Allow remote web access',
                })}
              </div>
              <div className="text-xs text-[var(--text-tertiary)]">
                {t('settings.remoteAccessToggleDescription', {
                  defaultValue:
                    'When off, the API binds to localhost only. Restart required after changes.',
                })}
              </div>
            </div>
            <Toggle
              checked={settings.backendRemoteEnabled}
              onChange={(checked: boolean) => updateSetting('backendRemoteEnabled', checked)}
            />
          </div>
          <div className="flex items-center justify-between" style={{ padding: '8px 0' }}>
            <div>
              <div className="text-sm font-medium text-[var(--text-primary)]">
                {t('settings.remoteAccessUiToggle', {
                  defaultValue: 'Serve web GUI from the host',
                })}
              </div>
              <div className="text-xs text-[var(--text-tertiary)]">
                {t('settings.remoteAccessUiToggleDescription', {
                  defaultValue:
                    'When off, the host will not serve the UI files. Restart required after changes.',
                })}
              </div>
            </div>
            <Toggle
              checked={settings.backendUiEnabled}
              onChange={(checked: boolean) => updateSetting('backendUiEnabled', checked)}
            />
          </div>
          <Input
            label={t('settings.remoteAccessHost', { defaultValue: 'Bind host' })}
            value={settings.backendHost}
            onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
              updateSetting('backendHost', e.target.value)
            }
            helper={t('settings.remoteAccessHostHelper', {
              defaultValue: 'Use 0.0.0.0 to listen on all interfaces.',
            })}
          />
          <Input
            label={t('settings.remoteAccessPort', { defaultValue: 'Port' })}
            type="number"
            value={settings.backendPort}
            min={1}
            max={65535}
            onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
              const value = Number(e.target.value);
              if (Number.isNaN(value)) return;
              updateSetting('backendPort', value);
            }}
          />
          <Input
            label={t('settings.remoteAccessToken', { defaultValue: 'Access token (optional)' })}
            type="password"
            value={settings.backendToken}
            onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
              updateSetting('backendToken', e.target.value)
            }
            helper={t('settings.remoteAccessTokenHelper', {
              defaultValue:
                'Clients must send this token as a Bearer auth header when enabled.',
            })}
          />
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
                onClick={() => dialogs.openExternal('https://github.com/ScopeCreep-zip/SpiritStream')}
              >
                <Github className="w-4 h-4" />
                {t('settings.github')}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() =>
                  dialogs.openExternal('https://deepwiki.com/ScopeCreep-zip/SpiritStream')
                }
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
