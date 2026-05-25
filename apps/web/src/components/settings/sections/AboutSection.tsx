import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Github, BookOpen, RefreshCw } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import { Logo } from '@/components/layout/Logo';
import { api } from '@/lib/client';

/**
 * Discriminated union of the self-updater state machine. Each variant
 * renders its own surface inside the card. `import('@tauri-apps/plugin-updater').Update`
 * carries the Tauri-side download/install handle while we're in the
 * 'available' state.
 */
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

/**
 * App version + GitHub/docs links + the self-updater state machine +
 * GPL-compliance third-party licenses modal entrypoint.
 *
 * `updaterSupported` is `true` on macOS / Windows / Linux AppImage, and
 * `false` on Linux .deb/.rpm installs (those are package-manager driven).
 */
export function AboutSection() {
  const { t } = useTranslation();
  const [appVersion, setAppVersion] = useState<string>('');
  const [updaterSupported, setUpdaterSupported] = useState<boolean | null>(null);
  const [updateState, setUpdateState] = useState<UpdateState>({ kind: 'idle' });
  const [licensesModalOpen, setLicensesModalOpen] = useState(false);

  // Probe app version + updater support once on mount; both are stable
  // for the lifetime of the process.
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

  const handleCheckForUpdates = useCallback(async (): Promise<void> => {
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
      api.system.recordAppUpdateFailure(detail).catch(() => {});
      setUpdateState({ kind: 'error', detail });
    }
  }, []);

  const handleInstallUpdate = useCallback(async (): Promise<void> => {
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
  }, [updateState]);

  const handleRelaunch = useCallback(async (): Promise<void> => {
    try {
      const { relaunch } = await import('@/utils/selfUpdate');
      await relaunch();
    } catch (e) {
      setUpdateState({
        kind: 'error',
        detail: e instanceof Error ? e.message : String(e),
      });
    }
  }, []);

  return (
    <>
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
                onClick={() =>
                  window.open(
                    'https://github.com/ScopeCreep-zip/SpiritStream',
                    '_blank',
                    'noopener,noreferrer',
                  )
                }
              >
                <Github className="w-4 h-4" />
                {t('settings.github')}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() =>
                  window.open(
                    'https://deepwiki.com/ScopeCreep-zip/SpiritStream',
                    '_blank',
                    'noopener,noreferrer',
                  )
                }
              >
                <BookOpen className="w-4 h-4" />
                {t('settings.docs')}
              </Button>
              {updaterSupported && (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={handleCheckForUpdates}
                  disabled={updateState.kind === 'checking' || updateState.kind === 'downloading'}
                >
                  <RefreshCw
                    className={`w-4 h-4 ${updateState.kind === 'checking' ? 'animate-spin' : ''}`}
                  />
                  {updateState.kind === 'checking'
                    ? t('settings.updateChecking', { defaultValue: 'Checking…' })
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
                {t('settings.thirdPartyLicenses', { defaultValue: 'Third-party licenses' })}
              </button>
            </div>
            {/* Update state machine surface — mutually exclusive states. */}
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
                      "Updates are managed by your distribution's package manager. Run `apt upgrade spiritstream` or `dnf upgrade spiritstream`.",
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
                  <Button variant="primary" size="sm" className="mt-3" onClick={handleInstallUpdate}>
                    {t('settings.updateInstall', { defaultValue: 'Download and install' })}
                  </Button>
                </div>
              )}
              {updateState.kind === 'downloading' && (
                <div className="text-text-secondary">
                  {t('settings.updateDownloading', { defaultValue: 'Downloading update…' })}
                  {updateState.total && updateState.total > 0 && (
                    <span>
                      {' '}
                      {Math.round((updateState.downloaded / updateState.total) * 100)}%
                    </span>
                  )}
                </div>
              )}
              {updateState.kind === 'ready-to-restart' && (
                <div className="rounded-md border border-status-info/40 bg-status-info/10 px-3 py-2">
                  <div className="text-status-info mb-2">
                    {t('settings.updateReady', {
                      defaultValue: 'Update installed. Restart to apply.',
                    })}
                  </div>
                  <Button variant="primary" size="sm" onClick={handleRelaunch}>
                    {t('settings.updateRestart', { defaultValue: 'Restart now' })}
                  </Button>
                </div>
              )}
              {updateState.kind === 'error' && (
                <div className="rounded-md border border-error-border bg-error-subtle px-3 py-2 text-left">
                  <div className="font-medium text-error-text mb-1">
                    {t('settings.updateErrorTitle', { defaultValue: 'Update check failed' })}
                  </div>
                  <div className="text-error-text text-xs break-words">{updateState.detail}</div>
                </div>
              )}
            </div>
          </div>
        </CardBody>
      </Card>

      {/* GPL compliance — Third-party licenses modal. FFmpeg is the
          load-bearing GPL dep (we ship the BtbN GPL build for hardware
          encoders on Windows / Linux, and evermeet's GPL build on
          macOS). The modal displays the bundled version + a link to
          the matching source tarball attached to the GitHub release. */}
      <Modal
        open={licensesModalOpen}
        onClose={() => setLicensesModalOpen(false)}
        title={t('settings.thirdPartyLicensesTitle', { defaultValue: 'Third-party licenses' })}
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
                    defaultValue: 'Matching source tarball (attached to this release)',
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
    </>
  );
}
