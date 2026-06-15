import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Select } from '@/components/ui/Select';
import { useThemeStore } from '@/stores/themeStore';
import { useProfileStore } from '@/stores/profileStore';
import { useFileBrowser } from '@/hooks/useFileBrowser';
import { api } from '@/lib/client';
import { logger } from '@/lib/logger';

const LANGUAGE_OPTIONS: Array<{ value: string; label: string }> = [
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

/**
 * Theme picker + theme installer + language picker for the active profile.
 * Subscribes directly to themeStore + profileStore; no orchestrator props.
 */
export function AppearanceSection() {
  const { t } = useTranslation();
  const { currentThemeId, themes, setTheme, refreshThemes, isInitialized, waitForInit } =
    useThemeStore();
  const currentProfile = useProfileStore((state) => state.current);
  const updateProfileSettings = useProfileStore((state) => state.updateProfileSettings);
  const profileSettings = currentProfile?.settings;
  const { FileBrowser, openFilePath: browserOpenFile } = useFileBrowser();

  const [themeInstalling, setThemeInstalling] = useState(false);
  const [themeInstallError, setThemeInstallError] = useState<string | null>(null);

  // The backend theme catalog loads asynchronously (App-level refreshThemes).
  // Until it's in, `themes` holds only the bundled set — rendering the picker
  // as enabled would let a user "pick" from a list that's silently missing
  // their installed/custom themes. Gate on init: `waitForInit` resolves on the
  // first catalog load (success OR failure), so this never hangs even offline.
  const [catalogReady, setCatalogReady] = useState(isInitialized);
  useEffect(() => {
    if (catalogReady) return;
    let cancelled = false;
    // `.catch` is defensive — `waitForInit` resolves on success OR failure, so
    // this clears the gate either way rather than leaving the picker disabled.
    waitForInit()
      .then(() => {
        if (!cancelled) setCatalogReady(true);
      })
      .catch(() => {
        if (!cancelled) setCatalogReady(true);
      });
    return () => {
      cancelled = true;
    };
  }, [catalogReady, waitForInit]);

  const themeOptions = (
    themes.length
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
  ).map((themeItem) => ({ value: themeItem.id, label: themeItem.name }));

  const handleThemeChange = useCallback(
    async (themeId: string): Promise<void> => {
      await setTheme(themeId);
      if (profileSettings) {
        await updateProfileSettings({ themeId });
      }
    },
    [setTheme, profileSettings, updateProfileSettings]
  );

  const handleInstallTheme = useCallback(async (): Promise<void> => {
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
  }, [browserOpenFile, refreshThemes, t]);

  const handleLanguageChange = useCallback(
    async (lang: string): Promise<void> => {
      if (!profileSettings) return;
      await updateProfileSettings({ language: lang });
    },
    [profileSettings, updateProfileSettings]
  );

  return (
    <Card>
      <FileBrowser />
      <CardHeader>
        <div>
          <CardTitle>{t('settings.appearance', { defaultValue: 'Appearance' })}</CardTitle>
          <CardDescription>
            {t('settings.appearanceDescription', {
              defaultValue: 'Theme and language for this profile.',
            })}
          </CardDescription>
        </div>
      </CardHeader>
      <CardBody className="p-6 flex flex-col gap-4">
        <Select
          label={t('settings.theme', { defaultValue: 'Theme' })}
          value={currentThemeId}
          disabled={!catalogReady}
          onChange={(e: React.ChangeEvent<HTMLSelectElement>) => handleThemeChange(e.target.value)}
          options={themeOptions}
          helper={
            catalogReady
              ? t('settings.themeHelper', {
                  defaultValue: 'Choose your preferred theme appearance.',
                })
              : t('settings.themeLoading', { defaultValue: 'Loading themes…' })
          }
        />
        <div className="flex items-center gap-3">
          <Button variant="outline" onClick={handleInstallTheme} disabled={themeInstalling}>
            {themeInstalling
              ? t('common.loading')
              : t('settings.installTheme', { defaultValue: 'Install Theme' })}
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
            handleLanguageChange(e.target.value)
          }
          options={LANGUAGE_OPTIONS}
        />
      </CardBody>
    </Card>
  );
}
