import { useTranslation } from 'react-i18next';
import { Download, X, CheckCircle, AlertCircle, Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { cn } from '@/lib/cn';
import { useFFmpegDownload, formatBytes, getPhaseLabel, type DownloadProgress } from '@/hooks/useFFmpegDownload';

interface FFmpegDownloadProgressProps {
  /** Callback when download completes successfully */
  onComplete?: (path: string) => void;
  /** Whether to show the download button initially */
  showDownloadButton?: boolean;
  /** Class name for the container */
  className?: string;
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
}: FFmpegDownloadProgressProps) {
  const { t } = useTranslation();
  const {
    progress,
    isDownloading,
    error,
    ffmpegPath,
    startDownload,
    cancelDownload,
  } = useFFmpegDownload();

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
    return (
      <div className={cn('flex items-center gap-2 text-sm text-[var(--success-text)]', className)}>
        <CheckCircle className="w-4 h-4" />
        <span>{t('settings.ffmpegInstalled')}</span>
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
    return (
      <div className={cn('space-y-3', className)}>
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2 text-sm text-[var(--text-secondary)]">
            <Loader2 className="w-4 h-4 animate-spin" />
            <span>{getPhaseLabel(progress.phase)}</span>
          </div>
          <Button variant="ghost" size="icon" onClick={cancelDownload}>
            <X className="w-4 h-4" />
          </Button>
        </div>

        <ProgressBar
          percent={progress.percent}
          downloaded={progress.downloaded}
          total={progress.total}
          phase={progress.phase}
        />
      </div>
    );
  }

  // Default: show download button
  if (showDownloadButton) {
    return (
      <Button
        variant="outline"
        size="sm"
        onClick={handleDownload}
        className={className}
      >
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
