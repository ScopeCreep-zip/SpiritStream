import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { RefreshCw } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { api } from '@/lib/client';

/**
 * Discriminated union of the self-updater state machine. Each variant
 * renders its own surface inside the card. The Tauri-side `Update`
 * handle carries the download/install state during 'available'.
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

export interface UpdaterController {
  updateState: UpdateState;
  handleCheckForUpdates: () => Promise<void>;
  handleInstallUpdate: () => Promise<void>;
  handleRelaunch: () => Promise<void>;
}

/**
 * The 7-state self-updater state machine, isolated from any rendering.
 * Returns the state + callbacks; pair with {@link UpdaterButton} and
 * {@link UpdaterStateView} (or build a custom surface) in the parent.
 */
export function useUpdaterController(): UpdaterController {
  const [updateState, setUpdateState] = useState<UpdateState>({ kind: 'idle' });

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

  return { updateState, handleCheckForUpdates, handleInstallUpdate, handleRelaunch };
}

interface UpdaterButtonProps {
  /** True on macOS / Windows / Linux AppImage; false on .deb/.rpm installs.
   *  When false the button renders null — the parent shows the
   *  distro-managed hint via {@link UpdaterStateView}. */
  updaterSupported: boolean | null;
  controller: UpdaterController;
}

/** Inline "Check for updates" button, designed to sit alongside the
 *  Github / Docs buttons in the About row. */
export function UpdaterButton({ updaterSupported, controller }: UpdaterButtonProps) {
  const { t } = useTranslation();
  if (!updaterSupported) return null;
  const { updateState, handleCheckForUpdates } = controller;
  return (
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
  );
}

interface UpdaterStateViewProps {
  updaterSupported: boolean | null;
  controller: UpdaterController;
}

/** Below-the-row status surface — renders one of seven mutually
 *  exclusive states (idle/no-update/distro-managed/available/
 *  downloading/ready-to-restart/error). */
export function UpdaterStateView({ updaterSupported, controller }: UpdaterStateViewProps) {
  const { t } = useTranslation();
  const { updateState, handleInstallUpdate, handleRelaunch } = controller;
  return (
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
  );
}
