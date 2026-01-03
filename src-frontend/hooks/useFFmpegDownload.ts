import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

/**
 * Download progress information from the backend
 */
export interface DownloadProgress {
  downloaded: number;
  total: number;
  percent: number;
  phase: 'starting' | 'downloading' | 'extracting' | 'verifying' | 'complete';
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
  /** Start the FFmpeg download */
  startDownload: () => Promise<void>;
  /** Cancel the current download */
  cancelDownload: () => Promise<void>;
  /** Check for an existing bundled FFmpeg */
  checkBundledFFmpeg: () => Promise<string | null>;
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

  // Listen to download progress events
  useEffect(() => {
    let unlistenProgress: UnlistenFn | null = null;

    const setupListener = async () => {
      unlistenProgress = await listen<DownloadProgress>('ffmpeg_download_progress', (event) => {
        setProgress(event.payload);

        // If download is complete, update the path
        if (event.payload.phase === 'complete') {
          setIsDownloading(false);
          // Check for the bundled path
          checkBundledFFmpeg().then(path => {
            if (path) setFFmpegPath(path);
          });
        }
      });
    };

    setupListener();

    return () => {
      if (unlistenProgress) unlistenProgress();
    };
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

  // Check for existing bundled FFmpeg on mount
  useEffect(() => {
    checkBundledFFmpeg();
  }, [checkBundledFFmpeg]);

  return {
    progress,
    isDownloading,
    error,
    ffmpegPath,
    startDownload,
    cancelDownload,
    checkBundledFFmpeg,
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
    case 'complete':
      return 'Complete!';
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
