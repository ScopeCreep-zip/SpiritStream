import { useTranslation } from 'react-i18next';
import { useEffect } from 'react';
import { Download, X, CheckCircle, AlertCircle, Loader2, ShieldCheck, RefreshCw } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { cn } from '@/lib/cn';
import {
  useFFmpegDownload,
  formatBytes,
  getPhaseLabel,
  type DownloadProgress,
} from '@/hooks/useFFmpegDownload';

interface FFmpegDownloadProgressProps {
  /** Callback when download completes successfully */
  onComplete?: (path: string) => void;
  /** Whether to show the download button initially */
  showDownloadButton?: boolean;
  /** Class name for the container */
  className?: string;
  /** Currently installed FFmpeg version (for update checking) */
  installedVersion?: string;
}

/**
 * FFmpeg download progress component
 *
 * Shows download button, progress bar, and status messages
 */
export function FFmpegDownloadProgress({
  onComplete,
  showDownloadButton = true,
  className,
  installedVersion,
}: FFmpegDownloadProgressProps) {
  const { t } = useTranslation();
  const {
    progress,
    isDownloading,
    error,
    ffmpegPath,
    versionInfo,
    isCheckingVersion,
    startDownload,
    cancelDownload,
    checkForUpdates,
  } = useFFmpegDownload();

  // Check for updates when we have an installed version
  useEffect(() => {
    if (installedVersion && ffmpegPath) {
      checkForUpdates(installedVersion);
    }
  }, [installedVersion, ffmpegPath, checkForUpdates]);

  const handleDownload = async () => {
    try {
      await startDownload();
      if (ffmpegPath && onComplete) {
        onComplete(ffmpegPath);
      }
    } catch {
      // Error is handled by the hook
    }
  };

  // If FFmpeg is already installed
  if (ffmpegPath && !isDownloading) {
    const hasUpdate = versionInfo?.update_available ?? false;
    const displayVersion = installedVersion || versionInfo?.installed_version;

    return (
      <div className={cn('space-y-2', className)}>
        <div className="flex items-center gap-2 text-sm text-[var(--success-text)]">
          <CheckCircle className="w-4 h-4" />
          <span>
            {t('settings.ffmpegInstalled')}
            {displayVersion && (
              <span className="text-[var(--text-secondary)]">
                {' '}(v{displayVersion.replace(/^ffmpeg\s+version\s+/i, '').split(' ')[0]})
              </span>
            )}
          </span>
        </div>

        {/* Update available notification */}
        {hasUpdate && versionInfo && (
          <div className="flex items-center justify-between gap-3 p-3 rounded-lg bg-[var(--warning-subtle)] border border-[var(--warning-border)]">
            <div className="flex items-center gap-2">
              <RefreshCw className="w-4 h-4 text-[var(--warning-text)]" />
              <div className="text-sm">
                <span className="font-medium text-[var(--warning-text)]">
                  {t('settings.updateAvailable')}
                </span>
                <span className="text-[var(--text-secondary)] ml-2">
                  {versionInfo.installed_version} → {versionInfo.latest_version}
                </span>
              </div>
            </div>
            <Button variant="outline" size="sm" onClick={handleDownload}>
              <Download className="w-4 h-4" />
              {t('settings.updateFFmpeg')}
            </Button>
          </div>
        )}

        {/* Checking for updates indicator */}
        {isCheckingVersion && (
          <div className="flex items-center gap-2 text-xs text-[var(--text-tertiary)]">
            <Loader2 className="w-3 h-3 animate-spin" />
            <span>{t('settings.checkingForUpdates')}</span>
          </div>
        )}
      </div>
    );
  }

  // If there's an error
  if (error && !isDownloading) {
    return (
      <div className={cn('space-y-3', className)}>
        <div className="flex items-center gap-2 text-sm text-[var(--error-text)]">
          <AlertCircle className="w-4 h-4" />
          <span>{error}</span>
        </div>
        {showDownloadButton && (
          <Button variant="outline" size="sm" onClick={handleDownload}>
            <Download className="w-4 h-4" />
            {t('settings.retryDownload')}
          </Button>
        )}
      </div>
    );
  }

  // If downloading
  if (isDownloading && progress) {
    const isRequestingPermission = progress.phase === 'requesting_permission';

    return (
      <div className={cn('space-y-3', className)}>
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2 text-sm text-[var(--text-secondary)]">
            {isRequestingPermission ? (
              <ShieldCheck className="w-4 h-4 text-[var(--primary)]" />
            ) : (
              <Loader2 className="w-4 h-4 animate-spin" />
            )}
            <span>{progress.message || getPhaseLabel(progress.phase)}</span>
          </div>
          {/* Hide cancel button during permission request */}
          {!isRequestingPermission && (
            <Button variant="ghost" size="icon" onClick={cancelDownload}>
              <X className="w-4 h-4" />
            </Button>
          )}
        </div>

        {isRequestingPermission ? (
          <p className="text-xs text-[var(--text-tertiary)]">
            {t('settings.elevationPromptHint')}
          </p>
        ) : (
          <ProgressBar
            percent={progress.percent}
            downloaded={progress.downloaded}
            total={progress.total}
            phase={progress.phase}
          />
        )}
      </div>
    );
  }

  // Default: show download button
  if (showDownloadButton) {
    return (
      <Button variant="outline" size="sm" onClick={handleDownload} className={className}>
        <Download className="w-4 h-4" />
        {t('settings.downloadFFmpeg')}
      </Button>
    );
  }

  return null;
}

interface ProgressBarProps {
  percent: number;
  downloaded: number;
  total: number;
  phase: DownloadProgress['phase'];
}

function ProgressBar({ percent, downloaded, total, phase }: ProgressBarProps) {
  const showBytes = phase === 'downloading' && total > 0;

  return (
    <div className="space-y-1">
      {/* Progress bar */}
      <div className="h-2 bg-[var(--bg-sunken)] rounded-full overflow-hidden">
        <div
          className="h-full bg-[var(--primary)] rounded-full transition-all duration-300"
          style={{ width: `${Math.min(percent, 100)}%` }}
        />
      </div>

      {/* Progress text */}
      <div className="flex items-center justify-between text-xs text-[var(--text-tertiary)]">
        {showBytes ? (
          <>
            <span>{formatBytes(downloaded)}</span>
            <span>{formatBytes(total)}</span>
          </>
        ) : (
          <>
            <span>{Math.round(percent)}%</span>
            <span>{getPhaseLabel(phase)}</span>
          </>
        )}
      </div>
    </div>
  );
}

export default FFmpegDownloadProgress;
