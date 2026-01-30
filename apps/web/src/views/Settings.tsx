import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderOpen, Download, Trash2, Github, BookOpen, RefreshCw } from 'lucide-react';
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
import { HotkeySettings } from '@/components/settings/HotkeySettings';
import { api, dialogs } from '@/lib/backend';
import { backendMode } from '@/lib/backend/env';
import { useFileBrowser } from '@/hooks/useFileBrowser';
import {
  useSettings,
  useFfmpegVersion,
  useUpdateSetting,
  useSaveSettings,
  useSettingsSync,
  useRefreshFfmpegVersion,
  SETTINGS_QUERY_KEY,
} from '@/hooks/useSettings';
import { useThemeStore } from '@/stores/themeStore';
import { useQueryClient } from '@tanstack/react-query';
import type { AppSettings } from '@/types/api';

export function Settings() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();

  // TanStack Query hooks for settings management
  const { data: settings, isLoading, isError } = useSettings();
  const { data: ffmpegData, isLoading: ffmpegLoading } = useFfmpegVersion();
  const updateSettingMutation = useUpdateSetting();
  const saveSettingsMutation = useSaveSettings();
  const refreshFfmpegVersion = useRefreshFfmpegVersion();

  // Sync with remote changes (only invalidates when not mutating)
  useSettingsSync();

  // Theme store
  const { currentThemeId, themes, setTheme, refreshThemes } = useThemeStore();

  // Local UI state (not server state)
  const [clearConfirmOpen, setClearConfirmOpen] = useState(false);
  const [clearInProgress, setClearInProgress] = useState(false);
  const [clearError, setClearError] = useState<string | null>(null);
  const [themeInstallError, setThemeInstallError] = useState<string | null>(null);
  const [themeInstalling, setThemeInstalling] = useState(false);

  // File browser hook for HTTP mode
  const {
    FileBrowser,
    openFilePath: browserOpenFile,
    openDirectoryPath: browserOpenDirectory,
  } = useFileBrowser();

  // Refresh themes on mount
  useEffect(() => {
    refreshThemes().catch(() => {
      // Errors are already logged in the store
    });
  }, [refreshThemes]);

  // Helper to update a single setting with optimistic update
  const updateSetting = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
    updateSettingMutation.mutate({ key, value });
  };

  const handleBrowseFfmpeg = async () => {
    try {
      const selected =
        backendMode === 'http'
          ? await browserOpenFile({
              filters: [{ name: 'FFmpeg', extensions: ['*'] }],
              initialPath: settings?.ffmpegPath || undefined,
            })
          : await dialogs.openFilePath({
              multiple: false,
              filters: [{ name: 'FFmpeg', extensions: ['*'] }],
            });
      if (!selected) {
        return;
      }
      if (typeof selected === 'string') {
        try {
          // Validate the path - throws if invalid
          await api.system.validateFfmpegPath(selected);
          // Path is valid, save it and refresh the version query
          saveSettingsMutation.mutate({ ffmpegPath: selected });
          refreshFfmpegVersion();
        } catch (validationError) {
          alert(`${t('settings.invalidFfmpegPath')}: ${validationError}`);
        }
      }
    } catch (error) {
      console.error('Failed to open file dialog:', error);
    }
  };

  const handleOpenProfileStorage = async () => {
    try {
      if (backendMode === 'http') {
        await browserOpenDirectory({
          title: t('settings.profileStorage'),
          initialPath: settings?.profileStoragePath,
        });
      } else {
        await dialogs.openExternal(settings?.profileStoragePath || '');
      }
    } catch (error) {
      console.error('Failed to open profile storage:', error);
    }
  };

  const handleExportData = async () => {
    try {
      const selected =
        backendMode === 'http'
          ? await browserOpenDirectory({
              title: t('settings.selectExportLocation'),
              initialPath: settings?.profileStoragePath || undefined,
            })
          : await dialogs.openDirectoryPath({
              multiple: false,
              title: t('settings.selectExportLocation'),
            });
      if (!selected) {
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
      const selected =
        backendMode === 'http'
          ? await browserOpenFile({
              filters: [{ name: 'Theme', extensions: ['json', 'jsonc'] }],
              title: t('settings.installTheme', { defaultValue: 'Install Theme' }),
            })
          : await dialogs.openFilePath({
              multiple: false,
              filters: [{ name: 'Theme', extensions: ['json', 'jsonc'] }],
              title: t('settings.installTheme', { defaultValue: 'Install Theme' }),
            });
      if (!selected) {
        setThemeInstalling(false);
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
      // Invalidate the settings query to reload defaults
      queryClient.invalidateQueries({ queryKey: SETTINGS_QUERY_KEY });
      setClearConfirmOpen(false);
    } catch (error) {
      console.error('Failed to clear data:', error);
      setClearError(`${t('settings.clearFailed')}: ${error}`);
    } finally {
      setClearInProgress(false);
    }
  };

  const handleThemeChange = async (themeId: string) => {
    await setTheme(themeId);
    // Save theme ID change to backend
    saveSettingsMutation.mutate({});
  };

  const languageOptions = [
    { value: 'en', label: 'English' },
    { value: 'es', label: 'Español' },
    { value: 'fr', label: 'Français' },
    { value: 'de', label: 'Deutsch' },
    { value: 'ja', label: '日本語' },
    { value: 'ar', label: 'العربية' },
    { value: 'zh-CN', label: '中文(简体)' },
    { value: 'ko', label: '한국어' },
    { value: 'uk', label: 'Українська' },
    { value: 'ru', label: 'Русский' },
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

  // Compute derived values
  const isSaving = updateSettingMutation.isPending || saveSettingsMutation.isPending;
  const ffmpegVersion = ffmpegData?.version || '';
  const ffmpegPath = settings?.ffmpegPath || ffmpegData?.path || '';

  // Loading state
  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-[var(--text-secondary)]">{t('common.loading')}</div>
      </div>
    );
  }

  // Error state
  if (isError || !settings) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-[var(--error-text)]">{t('settings.loadError', { defaultValue: 'Failed to load settings' })}</div>
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
                value={ffmpegPath}
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
              ffmpegLoading
                ? t('settings.detecting')
                : ffmpegVersion || t('settings.ffmpegNotFound')
            }
            disabled
            helper={t('settings.detectedVersion')}
          />

          {/* FFmpeg Status Section */}
          <div className="border-t border-[var(--border-muted)]" style={{ paddingTop: '16px' }}>
            <FFmpegDownloadProgress
              installedVersion={ffmpegVersion || undefined}
              autoDownload={settings.autoDownloadFfmpeg}
              onComplete={(path: string) => {
                // Save the new path
                saveSettingsMutation.mutate({ ffmpegPath: path });
                // Refresh FFmpeg version query
                refreshFfmpegVersion();
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
            disabled={isSaving}
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

      {/* Keyboard Shortcuts */}
      <HotkeySettings />

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

      {/* File browser modal for HTTP mode */}
      <FileBrowser />
    </Grid>
  );
}
