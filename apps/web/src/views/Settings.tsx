import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderOpen, Download, Trash2, Github, BookOpen, RefreshCw, Globe, User } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Select } from '@/components/ui/Select';
import { Input } from '@/components/ui/Input';
import { Toggle } from '@/components/ui/Toggle';
import { Grid } from '@/components/ui/Grid';
import { Modal } from '@/components/ui/Modal';
import { ConfirmDialog, PasswordInput } from '@spiritstream/ui';
import { Eye, EyeOff } from 'lucide-react';
import { Logo } from '@/components/layout/Logo';
import { KeyRotationSection } from '@/components/settings/KeyRotationSection';
import { api } from '@/lib/client';
import { useFileBrowser } from '@/hooks/useFileBrowser';
import {
  useSettings,
  useFfmpegVersion,
  useFfmpegUpdateCheck,
  useUpdateSetting,
  useSaveSettings,
  useSettingsSync,
  useRefreshFfmpegVersion,
  SETTINGS_QUERY_KEY,
} from '@/hooks/useSettings';
import { useThemeStore } from '@/stores/themeStore';
import { useProfileStore } from '@/stores/profileStore';
import { useQueryClient } from '@tanstack/react-query';
import type { Settings as AppSettings } from '@spiritstream/types';
import type { ProfileSettings as ProfileSettingsType, BackendSettings } from '@spiritstream/types';
import { logger } from '@/lib/logger';
import { cn } from '@/lib/cn';

type SettingsTab = 'global' | 'profile';

export function Settings() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();

  // Current tab (default to profile settings)
  const [activeTab, setActiveTab] = useState<SettingsTab>('profile');

  // TanStack Query hooks for global settings
  const { data: settings, isLoading, isError } = useSettings();
  const { data: ffmpegData, isLoading: ffmpegLoading } = useFfmpegVersion();
  const { data: ffmpegUpdate } = useFfmpegUpdateCheck(ffmpegData?.version);
  const updateSettingMutation = useUpdateSetting();
  const saveSettingsMutation = useSaveSettings();
  const refreshFfmpegVersion = useRefreshFfmpegVersion();

  // Profile store for profile settings
  const currentProfile = useProfileStore((state) => state.current);
  const updateProfileSettings = useProfileStore((state) => state.updateProfileSettings);
  const profileSettings = currentProfile?.settings;

  // Sync with remote changes
  useSettingsSync();

  // Theme store
  const { currentThemeId, themes, setTheme, refreshThemes } = useThemeStore();

  // Local UI state
  const [clearConfirmOpen, setClearConfirmOpen] = useState(false);
  const [clearInProgress, setClearInProgress] = useState(false);
  const [clearError, setClearError] = useState<string | null>(null);
  const [themeInstallError, setThemeInstallError] = useState<string | null>(null);
  const [themeInstalling, setThemeInstalling] = useState(false);
  // Updates state machine for the About card.
  // `appVersion` is the running build's version, fetched once from the
  // typed `/api/v1/system/app-version` route (no more hardcoded "0.1.0").
  // `updateState` is the user-visible status of the self-updater.
  const [appVersion, setAppVersion] = useState<string>('');
  const [updaterSupported, setUpdaterSupported] = useState<boolean | null>(null);
  type UpdateState =
    | { kind: 'idle' }
    | { kind: 'checking' }
    | { kind: 'no-update' }
    | {
        kind: 'available';
        version: string;
        notes: string;
        update: import('@tauri-apps/plugin-updater').Update;
      }
    | { kind: 'downloading'; downloaded: number; total: number | null }
    | { kind: 'ready-to-restart' }
    | { kind: 'error'; detail: string };
  const [updateState, setUpdateState] = useState<UpdateState>({ kind: 'idle' });
  // GPL compliance — viewing the third-party licenses page (currently
  // FFmpeg's GPL notice) is a user-visible obligation when the bundle
  // ships GPL builds. The modal is opened from the About card.
  const [licensesModalOpen, setLicensesModalOpen] = useState(false);

  // File browser hook for HTTP mode
  const {
    FileBrowser,
    openFilePath: browserOpenFile,
    openDirectoryPath: browserOpenDirectory,
  } = useFileBrowser();

  // Refresh themes on mount
  useEffect(() => {
    refreshThemes().catch(() => {});
  }, [refreshThemes]);

  // Fetch the running app version + probe whether the
  // updater is supported for this install (always true on macOS /
  // Windows; AppImage-only on Linux). Both fire once on mount and
  // never change for the lifetime of the process.
  useEffect(() => {
    let cancelled = false;
    api.system
      .appVersion()
      .then(({ version }) => {
        if (!cancelled) setAppVersion(version);
      })
      .catch(() => {});
    import('@/utils/selfUpdate')
      .then(({ isUpdaterSupported }) => isUpdaterSupported())
      .then((supported) => {
        if (!cancelled) setUpdaterSupported(supported);
      })
      .catch(() => {
        if (!cancelled) setUpdaterSupported(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const handleCheckForUpdates = async () => {
    setUpdateState({ kind: 'checking' });
    try {
      const { checkForUpdate } = await import('@/utils/selfUpdate');
      const update = await checkForUpdate();
      if (!update) {
        setUpdateState({ kind: 'no-update' });
        return;
      }
      setUpdateState({
        kind: 'available',
        version: update.version,
        notes: update.body ?? '',
        update,
      });
    } catch (e) {
      const detail = e instanceof Error ? e.message : String(e);
      // Security-relevant: record verification / network failures into
      // the audit chain so operators can grep for tampered-update
      // attempts. Best-effort — don't block the UI on the audit POST.
      void api.system.recordAppUpdateFailure(detail).catch(() => {});
      setUpdateState({ kind: 'error', detail });
    }
  };

  const handleInstallUpdate = async () => {
    if (updateState.kind !== 'available') return;
    const update = updateState.update;
    setUpdateState({ kind: 'downloading', downloaded: 0, total: null });
    try {
      const { downloadAndInstall } = await import('@/utils/selfUpdate');
      await downloadAndInstall(update, ({ downloaded, total }) => {
        setUpdateState({ kind: 'downloading', downloaded, total });
      });
      setUpdateState({ kind: 'ready-to-restart' });
    } catch (e) {
      setUpdateState({
        kind: 'error',
        detail: e instanceof Error ? e.message : String(e),
      });
    }
  };

  const handleRelaunch = async () => {
    try {
      const { relaunch } = await import('@/utils/selfUpdate');
      await relaunch();
    } catch (e) {
      setUpdateState({
        kind: 'error',
        detail: e instanceof Error ? e.message : String(e),
      });
    }
  };

  // Helper to update a global setting
  const updateGlobalSetting = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
    updateSettingMutation.mutate({ key, value });
  };

  // Helper to update a profile setting
  const updateProfileSetting = useCallback(
    async <K extends keyof ProfileSettingsType>(key: K, value: ProfileSettingsType[K]) => {
      if (!profileSettings) return;
      await updateProfileSettings({ [key]: value });
    },
    [profileSettings, updateProfileSettings]
  );

  // Helper to update nested backend settings
  const updateBackendSetting = useCallback(
    async <K extends keyof BackendSettings>(key: K, value: BackendSettings[K]) => {
      if (!profileSettings) return;
      await updateProfileSettings({
        backend: { ...profileSettings.backend, [key]: value },
      });
    },
    [profileSettings, updateProfileSettings]
  );

  const handleBrowseFfmpeg = async () => {
    try {
      const selected = await browserOpenFile({
        filters: [{ name: 'FFmpeg', extensions: ['*'] }],
        initialPath: settings?.ffmpegPath || undefined,
      });
      if (!selected) return;
      if (typeof selected === 'string') {
        try {
          await api.system.validateFfmpegPath(selected);
          saveSettingsMutation.mutate({ ffmpegPath: selected });
          refreshFfmpegVersion();
        } catch (validationError) {
          alert(`${t('settings.invalidFfmpegPath')}: ${validationError}`);
        }
      }
    } catch (error) {
      logger.error('Failed to open file dialog:', error);
    }
  };

  const handleOpenProfileStorage = async () => {
    try {
      await browserOpenDirectory({
        title: t('settings.profileStorage'),
        initialPath: settings?.profileStoragePath,
      });
    } catch (error) {
      logger.error('Failed to open profile storage:', error);
    }
  };

  const handleExportData = async () => {
    try {
      const selected = await browserOpenDirectory({
        title: t('settings.selectExportLocation'),
        initialPath: settings?.profileStoragePath || undefined,
      });
      if (!selected) return;
      await api.settings.exportData(selected);
      alert(t('toast.dataExported'));
    } catch (error) {
      logger.error('Failed to export data:', error);
      alert(`${t('settings.exportFailed')}: ${error}`);
    }
  };

  const handleInstallTheme = async () => {
    setThemeInstallError(null);
    setThemeInstalling(true);
    try {
      const selected = await browserOpenFile({
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
      logger.error('Failed to install theme:', error);
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
      queryClient.invalidateQueries({ queryKey: SETTINGS_QUERY_KEY });
      setClearConfirmOpen(false);
    } catch (error) {
      logger.error('Failed to clear data:', error);
      setClearError(`${t('settings.clearFailed')}: ${error}`);
    } finally {
      setClearInProgress(false);
    }
  };

  const handleThemeChange = async (themeId: string) => {
    await setTheme(themeId);
    // Also save to profile settings
    if (profileSettings) {
      await updateProfileSettings({ themeId });
    }
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
        { id: 'spirit-light', name: 'Spirit Light', mode: 'light' as const, source: 'builtin' as const },
        { id: 'spirit-dark', name: 'Spirit Dark', mode: 'dark' as const, source: 'builtin' as const },
      ]
  ).map((themeItem) => ({
    value: themeItem.id,
    label: themeItem.name,
  }));

  const isSaving = updateSettingMutation.isPending || saveSettingsMutation.isPending;
  const ffmpegVersion = ffmpegData?.version || '';
  const ffmpegPath = settings?.ffmpegPath || ffmpegData?.path || '';

  // Loading state
  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-text-secondary">{t('common.loading')}</div>
      </div>
    );
  }

  // Error state
  if (isError || !settings) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-error-text">{t('settings.loadError', { defaultValue: 'Failed to load settings' })}</div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Tab Navigation */}
      <div className="flex gap-2 border-b border-border-default pb-2">
        <button
          onClick={() => setActiveTab('profile')}
          className={cn(
            'flex items-center gap-2 px-4 py-2 rounded-t-lg text-sm font-medium transition-colors',
            activeTab === 'profile'
              ? 'bg-bg-surface text-text-primary border border-b-0 border-border-default'
              : 'text-text-secondary hover:text-text-primary hover:bg-bg-muted'
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
              : 'text-text-secondary hover:text-text-primary hover:bg-bg-muted'
          )}
        >
          <Globe className="w-4 h-4" />
          {t('settings.globalSettings', { defaultValue: 'Global Settings' })}
        </button>
      </div>

      {/* Profile Settings Tab */}
      {activeTab === 'profile' && (
        <>
          {!currentProfile ? (
            <div className="flex items-center justify-center h-64 text-text-tertiary">
              {t('settings.loadProfileFirst', { defaultValue: 'Please load a profile to edit its settings.' })}
            </div>
          ) : (
            <Grid cols={2}>
              {/* Appearance */}
              <Card>
                <CardHeader>
                  <div>
                    <CardTitle>{t('settings.appearance', { defaultValue: 'Appearance' })}</CardTitle>
                    <CardDescription>{t('settings.appearanceDescription', { defaultValue: 'Theme and language for this profile.' })}</CardDescription>
                  </div>
                </CardHeader>
                <CardBody className="p-6 flex flex-col gap-4">
                  <Select
                    label={t('settings.theme', { defaultValue: 'Theme' })}
                    value={currentThemeId}
                    onChange={(e: React.ChangeEvent<HTMLSelectElement>) => handleThemeChange(e.target.value)}
                    options={themeOptions}
                    helper={t('settings.themeHelper', { defaultValue: 'Choose your preferred theme appearance.' })}
                  />
                  <div className="flex items-center gap-3">
                    <Button variant="outline" onClick={handleInstallTheme} disabled={themeInstalling}>
                      {themeInstalling ? t('common.loading') : t('settings.installTheme', { defaultValue: 'Install Theme' })}
                    </Button>
                  </div>
                  {themeInstallError && (
                    <div className="p-3 rounded-lg bg-error-subtle border border-error-border">
                      <p className="text-sm text-error-text">{themeInstallError}</p>
                    </div>
                  )}
                  <Select
                    label={t('settings.language')}
                    value={profileSettings?.language || 'en'}
                    onChange={(e: React.ChangeEvent<HTMLSelectElement>) =>
                      updateProfileSetting('language', e.target.value)
                    }
                    options={languageOptions}
                  />
                </CardBody>
              </Card>

              {/* Notifications & Security */}
              <Card>
                <CardHeader>
                  <div>
                    <CardTitle>{t('settings.notificationsSecurity', { defaultValue: 'Notifications & Security' })}</CardTitle>
                    <CardDescription>{t('settings.notificationsSecurityDescription', { defaultValue: 'Notification preferences and data security for this profile.' })}</CardDescription>
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

              {/* Remote Access */}
              <Card className="col-span-2">
                <CardHeader>
                  <div>
                    <CardTitle>{t('settings.remoteAccess', { defaultValue: 'Remote Access' })}</CardTitle>
                    <CardDescription>
                      {t('settings.remoteAccessDescription', {
                        defaultValue: 'Enable the built-in HTTP API so you can manage SpiritStream from another device.',
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
                              defaultValue: 'When off, the API binds to localhost only. Restart required after changes.',
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
                            {t('settings.remoteAccessUiToggle', { defaultValue: 'Serve web GUI from the host' })}
                          </div>
                          <div className="text-xs text-text-tertiary">
                            {t('settings.remoteAccessUiToggleDescription', {
                              defaultValue: 'When off, the host will not serve the UI files. Restart required after changes.',
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
                          defaultValue: 'Clients must send this token as a Bearer auth header when enabled.',
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
            </Grid>
          )}
        </>
      )}

      {/* Global Settings Tab */}
      {activeTab === 'global' && (
        <Grid cols={2}>
          {/* FFmpeg Configuration */}
          <Card>
            <CardHeader>
              <div>
                <CardTitle>{t('settings.ffmpegConfig')}</CardTitle>
                <CardDescription>{t('settings.ffmpegDescription')}</CardDescription>
              </div>
            </CardHeader>
            <CardBody className="p-6 flex flex-col gap-4">
              <div className="flex flex-col gap-1.5">
                <label className="block text-sm font-medium text-text-primary">
                  {t('settings.ffmpegPath')}
                </label>
                <div className="flex gap-2">
                  <Input
                    value={ffmpegPath}
                    onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                      updateGlobalSetting('ffmpegPath', e.target.value)
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
              {ffmpegVersion && ffmpegUpdate?.updateAvailable && (
                <div
                  className="rounded-md border border-status-warning/40 bg-status-warning/10 px-3 py-2 text-sm text-status-warning"
                  role="status"
                >
                  {t('settings.ffmpegUpdateAvailable', {
                    defaultValue: 'Update available: {{installed}} → {{latest}}',
                    installed: ffmpegUpdate.installedVersion ?? ffmpegVersion,
                    latest: ffmpegUpdate.latestVersion ?? '?',
                  })}
                </div>
              )}
              {/*
                FFmpeg is delivered per-platform at build time:
                  - macOS / Windows: bundled as a pinned Tauri 2 sidecar
                    (version pinned in scripts/ffmpeg-pins.json, SHA-256
                    verified at build time)
                  - Linux .deb / .rpm: distro `ffmpeg` package (.deb / .rpm
                    dep) — gets distro security updates
                  - Linux AppImage: bundled pinned static build
                  - Docker / CLI: $PATH (admin installs once)
                To update FFmpeg the user updates the app — the new release
                ships a new pinned version. Use the Updates button in the
                About section below.
              */}
            </CardBody>
          </Card>

          {/* App Behavior */}
          <Card>
            <CardHeader>
              <div>
                <CardTitle>{t('settings.appBehavior', { defaultValue: 'App Behavior' })}</CardTitle>
                <CardDescription>{t('settings.appBehaviorDescription', { defaultValue: 'System-wide application settings.' })}</CardDescription>
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

          {/* Data Management */}
          <Card>
            <CardHeader>
              <div>
                <CardTitle>{t('settings.dataManagement', { defaultValue: 'Data Management' })}</CardTitle>
                <CardDescription>{t('settings.dataManagementDescription', { defaultValue: 'Export and manage your application data.' })}</CardDescription>
              </div>
            </CardHeader>
            <CardBody className="p-6 flex flex-col gap-4">
              <div className="flex flex-col gap-1.5">
                <label className="block text-sm font-medium text-text-primary">
                  {t('settings.profileStorage')}
                </label>
                <div className="flex gap-2">
                  <Input value={settings.profileStoragePath} disabled className="flex-1 font-mono text-xs" />
                  <Button variant="outline" onClick={handleOpenProfileStorage}>
                    <FolderOpen className="w-4 h-4" />
                    {t('settings.open')}
                  </Button>
                </div>
              </div>
              <div className="flex gap-3">
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
              <div className="text-center py-4">
                <div className="flex justify-center mb-4">
                  <Logo size="lg" />
                </div>
                <div className="text-sm text-text-secondary mb-1">
                  {t('settings.version')} {appVersion || '…'}
                </div>
                <div className="text-xs text-text-tertiary mb-6">
                  {t('settings.tagline')}
                </div>
                <div className="flex justify-center gap-3">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => window.open('https://github.com/ScopeCreep-zip/SpiritStream', '_blank', 'noopener,noreferrer')}
                  >
                    <Github className="w-4 h-4" />
                    {t('settings.github')}
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => window.open('https://deepwiki.com/ScopeCreep-zip/SpiritStream', '_blank', 'noopener,noreferrer')}
                  >
                    <BookOpen className="w-4 h-4" />
                    {t('settings.docs')}
                  </Button>
                  {updaterSupported && (
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={handleCheckForUpdates}
                      disabled={
                        updateState.kind === 'checking' ||
                        updateState.kind === 'downloading'
                      }
                    >
                      <RefreshCw
                        className={`w-4 h-4 ${updateState.kind === 'checking' ? 'animate-spin' : ''}`}
                      />
                      {updateState.kind === 'checking'
                        ? t('settings.updateChecking', {
                            defaultValue: 'Checking…',
                          })
                        : t('settings.updates')}
                    </Button>
                  )}
                </div>
                <div className="mt-3">
                  <button
                    type="button"
                    className="text-xs text-text-tertiary underline hover:text-text-secondary"
                    onClick={() => setLicensesModalOpen(true)}
                  >
                    {t('settings.thirdPartyLicenses', {
                      defaultValue: 'Third-party licenses',
                    })}
                  </button>
                </div>
                {/* Update state machine surface — sits below the action
                    buttons inside the About card. Mutually exclusive
                    states, so we render one block per state. */}
                <div className="mt-4 text-xs">
                  {updateState.kind === 'no-update' && (
                    <div className="text-text-tertiary">
                      {t('settings.updateNone', {
                        defaultValue: 'You are running the latest version.',
                      })}
                    </div>
                  )}
                  {updaterSupported === false && (
                    <div className="text-text-tertiary">
                      {t('settings.updateDistroManaged', {
                        defaultValue:
                          'Updates are managed by your distribution\'s package manager. Run `apt upgrade spiritstream` or `dnf upgrade spiritstream`.',
                      })}
                    </div>
                  )}
                  {updateState.kind === 'available' && (
                    <div className="rounded-md border border-status-info/40 bg-status-info/10 px-3 py-2 text-left">
                      <div className="font-medium text-status-info mb-1">
                        {t('settings.updateAvailableTitle', {
                          defaultValue: 'Update available: {{version}}',
                          version: updateState.version,
                        })}
                      </div>
                      {updateState.notes && (
                        <pre className="whitespace-pre-wrap text-text-secondary text-xs mt-2 max-h-32 overflow-auto">
                          {updateState.notes}
                        </pre>
                      )}
                      <Button
                        variant="primary"
                        size="sm"
                        className="mt-3"
                        onClick={handleInstallUpdate}
                      >
                        {t('settings.updateInstall', {
                          defaultValue: 'Download and install',
                        })}
                      </Button>
                    </div>
                  )}
                  {updateState.kind === 'downloading' && (
                    <div className="text-text-secondary">
                      {t('settings.updateDownloading', {
                        defaultValue: 'Downloading update…',
                      })}
                      {updateState.total && updateState.total > 0 && (
                        <span>
                          {' '}
                          {Math.round(
                            (updateState.downloaded / updateState.total) * 100,
                          )}
                          %
                        </span>
                      )}
                    </div>
                  )}
                  {updateState.kind === 'ready-to-restart' && (
                    <div className="rounded-md border border-status-info/40 bg-status-info/10 px-3 py-2">
                      <div className="text-status-info mb-2">
                        {t('settings.updateReady', {
                          defaultValue:
                            'Update installed. Restart to apply.',
                        })}
                      </div>
                      <Button variant="primary" size="sm" onClick={handleRelaunch}>
                        {t('settings.updateRestart', {
                          defaultValue: 'Restart now',
                        })}
                      </Button>
                    </div>
                  )}
                  {updateState.kind === 'error' && (
                    <div className="rounded-md border border-error-border bg-error-subtle px-3 py-2 text-left">
                      <div className="font-medium text-error-text mb-1">
                        {t('settings.updateErrorTitle', {
                          defaultValue: 'Update check failed',
                        })}
                      </div>
                      <div className="text-error-text text-xs break-words">
                        {updateState.detail}
                      </div>
                    </div>
                  )}
                </div>
              </div>
            </CardBody>
          </Card>
        </Grid>
      )}

      {/* Clear Data — ConfirmDialog with confirm-token flow. */}
      <ConfirmDialog
        open={clearConfirmOpen}
        title={t('settings.clearAllData')}
        confirmLabel={clearInProgress ? t('common.loading') : t('common.confirm')}
        cancelLabel={t('common.cancel')}
        confirmDisabled={clearInProgress}
        onConfirm={handleClearConfirm}
        onCancel={handleClearCancel}
        message={
          <>
            <p>{t('settings.clearConfirm')}</p>
            {clearError && (
              <div className="mt-4 p-3 rounded-lg bg-error-subtle border border-error-border">
                <p className="text-sm text-error-text">{clearError}</p>
              </div>
            )}
          </>
        }
      />

      {/* GPL compliance — Third-party licenses modal. FFmpeg is the
          load-bearing GPL dep (we ship the BtbN GPL build for hardware
          encoders on Windows / Linux, and evermeet's GPL build on
          macOS). The modal displays the bundled version + a link to
          the matching source tarball attached to the GitHub release. */}
      <Modal
        open={licensesModalOpen}
        onClose={() => setLicensesModalOpen(false)}
        title={t('settings.thirdPartyLicensesTitle', {
          defaultValue: 'Third-party licenses',
        })}
        footer={
          <Button variant="ghost" onClick={() => setLicensesModalOpen(false)}>
            {t('common.close', { defaultValue: 'Close' })}
          </Button>
        }
      >
        <div className="flex flex-col gap-4 text-sm">
          <p className="text-text-secondary">
            {t('settings.thirdPartyLicensesIntro', {
              defaultValue:
                'SpiritStream bundles the following third-party software. License notices and corresponding source code are linked below.',
            })}
          </p>
          <div className="rounded-md border border-border-subtle p-3">
            <div className="flex items-baseline justify-between gap-2">
              <div className="font-medium">FFmpeg</div>
              <div className="text-xs text-text-tertiary">GPL v2 or later</div>
            </div>
            <div className="text-xs text-text-tertiary mt-1">
              {t('settings.thirdPartyLicensesFfmpegBundled', {
                defaultValue: 'Bundled with this build of SpiritStream.',
              })}
            </div>
            <ul className="mt-3 text-xs flex flex-col gap-1">
              <li>
                <a
                  href="https://www.ffmpeg.org/legal.html"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-accent-text underline"
                >
                  {t('settings.thirdPartyLicensesFfmpegLicense', {
                    defaultValue: 'License (ffmpeg.org/legal.html)',
                  })}
                </a>
              </li>
              <li>
                <a
                  href={`https://github.com/ScopeCreep-zip/SpiritStream/releases/tag/v${appVersion || '0.0.0'}`}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-accent-text underline"
                >
                  {t('settings.thirdPartyLicensesFfmpegSource', {
                    defaultValue:
                      'Matching source tarball (attached to this release)',
                  })}
                </a>
              </li>
              <li>
                <a
                  href="https://www.ffmpeg.org/download.html"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-accent-text underline"
                >
                  {t('settings.thirdPartyLicensesFfmpegUpstream', {
                    defaultValue: 'Upstream binary distributors',
                  })}
                </a>
              </li>
            </ul>
          </div>
        </div>
      </Modal>

      {/* File browser modal for HTTP mode */}
      <FileBrowser />
    </div>
  );
}
