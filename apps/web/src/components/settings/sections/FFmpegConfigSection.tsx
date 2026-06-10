import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderOpen } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { FormGroup, FormLabel } from '@/components/ui/Form';
import {
  useFfmpegUpdateCheck,
  useFfmpegVersion,
  useRefreshFfmpegVersion,
  useSaveSettings,
  useSettings,
} from '@/hooks/useSettings';
import { useFileBrowser } from '@/hooks/useFileBrowser';
import { api } from '@/lib/client';
import { logger } from '@/lib/logger';

/**
 * Global FFmpeg path / version / update-available card. Deliberately
 * keeps the comment about how FFmpeg is delivered per-platform — that's
 * load-bearing context for maintainers.
 */
export function FFmpegConfigSection() {
  const { t } = useTranslation();
  const { data: settings } = useSettings();
  const { data: ffmpegData, isLoading: ffmpegLoading } = useFfmpegVersion();
  const { data: ffmpegUpdate } = useFfmpegUpdateCheck(ffmpegData?.version);
  const saveSettingsMutation = useSaveSettings();
  const refreshFfmpegVersion = useRefreshFfmpegVersion();
  const { FileBrowser, openFilePath: browserOpenFile } = useFileBrowser();

  const ffmpegVersion = ffmpegData?.version || '';
  const ffmpegPath = settings?.ffmpegPath || ffmpegData?.path || '';

  const handlePathChange = useCallback(
    (event: React.ChangeEvent<HTMLInputElement>) => {
      saveSettingsMutation.mutate({ ffmpegPath: event.target.value });
    },
    [saveSettingsMutation]
  );

  const handleBrowse = useCallback(async () => {
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
  }, [browserOpenFile, settings, saveSettingsMutation, refreshFfmpegVersion, t]);

  return (
    <Card>
      <FileBrowser />
      <CardHeader>
        <div>
          <CardTitle>{t('settings.ffmpegConfig')}</CardTitle>
          <CardDescription>{t('settings.ffmpegDescription')}</CardDescription>
        </div>
      </CardHeader>
      <CardBody className="p-6 flex flex-col gap-4">
        <FormGroup>
          <FormLabel>{t('settings.ffmpegPath')}</FormLabel>
          <div className="flex gap-2">
            <Input value={ffmpegPath} onChange={handlePathChange} className="flex-1" />
            <Button variant="outline" onClick={handleBrowse}>
              <FolderOpen className="w-4 h-4" />
              {t('settings.browse')}
            </Button>
          </div>
        </FormGroup>
        <Input
          label={t('settings.ffmpegVersion')}
          value={
            ffmpegLoading ? t('settings.detecting') : ffmpegVersion || t('settings.ffmpegNotFound')
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
  );
}
