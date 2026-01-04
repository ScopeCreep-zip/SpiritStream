import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderOpen, Download, Trash2, Github, BookOpen, RefreshCw } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Select } from '@/components/ui/Select';
import { Input } from '@/components/ui/Input';
import { Toggle } from '@/components/ui/Toggle';
import { Grid } from '@/components/ui/Grid';
import { Logo } from '@/components/layout/Logo';
import { FFmpegDownloadProgress } from '@/components/settings/FFmpegDownloadProgress';
import { api } from '@/lib/tauri';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { open as openPath } from '@tauri-apps/plugin-shell';
import { useLanguageStore, type Language } from '@/stores/languageStore';
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
  loading: true,
  saving: false,
};

export function Settings() {
  const { t } = useTranslation();
  const { setLanguage, initFromSettings } = useLanguageStore();
  const [settings, setSettings] = useState<SettingsState>(defaultSettings);

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

  const handleClearAllData = async () => {
    if (confirm(t('settings.clearConfirm'))) {
      try {
        await api.settings.clearData();
        alert(t('toast.dataCleared'));
        // Reset to defaults
        setSettings({ ...defaultSettings, loading: false, saving: false });
      } catch (error) {
        console.error('Failed to clear data:', error);
        alert(`${t('settings.clearFailed')}: ${error}`);
      }
    }
  };

  const languageOptions = [
    { value: 'en', label: 'English' },
    { value: 'es', label: 'Español' },
    { value: 'fr', label: 'Français' },
    { value: 'de', label: 'Deutsch' },
    { value: 'ja', label: '日本語' },
  ];

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
                onClick={() => window.open('https://github.com/ScopeCreep-zip/SpiritStream', '_blank')}
              >
                <Github className="w-4 h-4" />
                {t('settings.github')}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() =>
                  window.open('https://github.com/ScopeCreep-zip/SpiritStream#readme', '_blank')
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
    </Grid>
  );
}
