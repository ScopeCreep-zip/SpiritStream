import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import type { FFmpegVersionInfo } from '@/types/api';

/**
 * Download progress information from the backend
 */
export interface DownloadProgress {
  downloaded: number;
  total: number;
  percent: number;
  phase: 'starting' | 'downloading' | 'extracting' | 'verifying' | 'requesting_permission' | 'complete' | 'elevation_denied' | 'error';
  message?: string;
}

/**
 * State returned by the useFFmpegDownload hook
 */
export interface FFmpegDownloadState {
  /** Current download progress */
  progress: DownloadProgress | null;
  /** Whether a download is in progress */
  isDownloading: boolean;
  /** Error message if download failed */
  error: string | null;
  /** Path to the downloaded FFmpeg binary */
  ffmpegPath: string | null;
  /** Version info including update availability */
  versionInfo: FFmpegVersionInfo | null;
  /** Whether version check is in progress */
  isCheckingVersion: boolean;
  /** Start the FFmpeg download */
  startDownload: () => Promise<void>;
  /** Cancel the current download */
  cancelDownload: () => Promise<void>;
  /** Check for an existing bundled FFmpeg */
  checkBundledFFmpeg: () => Promise<string | null>;
  /** Check for FFmpeg updates */
  checkForUpdates: (installedVersion?: string) => Promise<FFmpegVersionInfo | null>;
}

/**
 * Hook to manage FFmpeg auto-download
 *
 * @example
 * ```tsx
 * function FFmpegSettings() {
 *   const { progress, isDownloading, error, startDownload, cancelDownload } = useFFmpegDownload();
 *
 *   return (
 *     <div>
 *       {isDownloading && <ProgressBar percent={progress?.percent || 0} />}
 *       {error && <Alert variant="error">{error}</Alert>}
 *       <Button onClick={startDownload} disabled={isDownloading}>
 *         Download FFmpeg
 *       </Button>
 *     </div>
 *   );
 * }
 * ```
 */
export function useFFmpegDownload(): FFmpegDownloadState {
  const [progress, setProgress] = useState<DownloadProgress | null>(null);
  const [isDownloading, setIsDownloading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [ffmpegPath, setFFmpegPath] = useState<string | null>(null);
  const [versionInfo, setVersionInfo] = useState<FFmpegVersionInfo | null>(null);
  const [isCheckingVersion, setIsCheckingVersion] = useState(false);

  // Listen to download progress events
  useEffect(() => {
    let unlistenProgress: UnlistenFn | null = null;

    const setupListener = async () => {
      unlistenProgress = await listen<DownloadProgress>('ffmpeg_download_progress', (event) => {
        setProgress(event.payload);

        // Handle completion states
        if (event.payload.phase === 'complete') {
          setIsDownloading(false);
          setError(null);
          // Check for the bundled path
          checkBundledFFmpeg().then((path) => {
            if (path) setFFmpegPath(path);
          });
        } else if (event.payload.phase === 'elevation_denied') {
          setIsDownloading(false);
          setError(event.payload.message || 'Permission denied');
        } else if (event.payload.phase === 'error') {
          setIsDownloading(false);
          setError(event.payload.message || 'Installation failed');
        }
      });
    };

    setupListener();

    return () => {
      if (unlistenProgress) unlistenProgress();
    };
    // checkBundledFFmpeg is intentionally excluded - we only want to set up the listener once
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Check for an existing bundled FFmpeg
  const checkBundledFFmpeg = useCallback(async (): Promise<string | null> => {
    try {
      const path = await invoke<string | null>('get_bundled_ffmpeg_path');
      setFFmpegPath(path);
      return path;
    } catch (err) {
      console.error('Failed to check bundled FFmpeg:', err);
      return null;
    }
  }, []);

  // Start the FFmpeg download
  const startDownload = useCallback(async (): Promise<void> => {
    setIsDownloading(true);
    setError(null);
    setProgress({
      downloaded: 0,
      total: 0,
      percent: 0,
      phase: 'starting',
    });

    try {
      const path = await invoke<string>('download_ffmpeg');
      setFFmpegPath(path);
      setIsDownloading(false);
    } catch (err) {
      const errorMessage = String(err);
      setError(errorMessage);
      setIsDownloading(false);
      throw err;
    }
  }, []);

  // Cancel the current download
  const cancelDownload = useCallback(async (): Promise<void> => {
    try {
      await invoke('cancel_ffmpeg_download');
      setIsDownloading(false);
      setProgress(null);
    } catch (err) {
      console.error('Failed to cancel download:', err);
    }
  }, []);

  // Check for FFmpeg updates
  const checkForUpdates = useCallback(
    async (installedVersion?: string): Promise<FFmpegVersionInfo | null> => {
      setIsCheckingVersion(true);
      try {
        const info = await invoke<FFmpegVersionInfo>('check_ffmpeg_update', {
          installedVersion,
        });
        setVersionInfo(info);
        return info;
      } catch (err) {
        console.error('Failed to check for FFmpeg updates:', err);
        return null;
      } finally {
        setIsCheckingVersion(false);
      }
    },
    []
  );

  // Check for existing bundled FFmpeg on mount
  useEffect(() => {
    checkBundledFFmpeg();
  }, [checkBundledFFmpeg]);

  return {
    progress,
    isDownloading,
    error,
    ffmpegPath,
    versionInfo,
    isCheckingVersion,
    startDownload,
    cancelDownload,
    checkBundledFFmpeg,
    checkForUpdates,
  };
}

/**
 * Get the phase label for display
 */
export function getPhaseLabel(phase: DownloadProgress['phase']): string {
  switch (phase) {
    case 'starting':
      return 'Starting...';
    case 'downloading':
      return 'Downloading...';
    case 'extracting':
      return 'Extracting...';
    case 'verifying':
      return 'Verifying...';
    case 'requesting_permission':
      return 'Requesting permission...';
    case 'complete':
      return 'Complete!';
    case 'elevation_denied':
      return 'Permission denied';
    case 'error':
      return 'Error';
    default:
      return 'Processing...';
  }
}

/**
 * Format bytes to human-readable size
 */
export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}
