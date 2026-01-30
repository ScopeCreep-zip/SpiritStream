import { useEffect, useRef, useCallback } from 'react';
import { events, api } from '@/lib/backend';
import { useStreamStore } from '@/stores/streamStore';
import { toast } from '@/hooks/useToast';
import { useTranslation } from 'react-i18next';

/**
 * Stream statistics from FFmpeg
 */
export interface StreamStats {
  groupId: string;
  frame: number;
  fps: number;
  bitrate: number;
  speed: number;
  size: number;
  time: number;
  droppedFrames: number;
  dupFrames: number;
}

/**
 * Stream error from FFmpeg crash
 */
export interface StreamError {
  groupId: string;
  error: string;
  canRetry: boolean;
  suggestion?: string;
}

/**
 * Stream reconnecting event from backend
 */
export interface StreamReconnecting {
  groupId: string;
  attempt: number;
  maxAttempts: number;
  delaySecs: number;
}

/**
 * Hook to listen to real-time stream statistics from the Rust backend
 * Also handles automatic retry on stream errors
 */
export function useStreamStats() {
  const { t } = useTranslation();
  const { updateStats, setStreamEnded, setStreamError } = useStreamStore();

  // Track groups currently being retried to prevent duplicate retry attempts
  const retryingGroups = useRef<Set<string>>(new Set());

  // Handle auto-retry for a failed stream
  const handleAutoRetry = useCallback(async (groupId: string) => {
    // Prevent concurrent retry attempts for the same group
    if (retryingGroups.current.has(groupId)) {
      return;
    }

    retryingGroups.current.add(groupId);

    try {
      // Show toast that we're retrying
      toast.info(t('stream.autoRetrying', 'Stream disconnected, reconnecting...'));

      const result = await api.stream.retry(groupId);

      if (result.pid > 0) {
        // Retry succeeded
        toast.success(t('stream.retrySuccess', 'Stream reconnected successfully'));
      }
    } catch (error) {
      // Retry failed - show error but don't spam
      const message = error instanceof Error ? error.message : String(error);
      toast.error(t('stream.retryFailed', 'Reconnection failed: {{error}}', { error: message }));
    } finally {
      retryingGroups.current.delete(groupId);
    }
  }, [t]);

  // Set up event listeners
  useEffect(() => {
    let unlistenStats: (() => void) | null = null;
    let unlistenEnded: (() => void) | null = null;
    let unlistenError: (() => void) | null = null;
    let unlistenReconnecting: (() => void) | null = null;

    const setupListeners = async () => {
      // Listen for stream stats updates
      unlistenStats = await events.on<StreamStats>('stream_stats', (payload) => {
        updateStats(payload.groupId, payload);
      });

      // Listen for stream ended events (clean exit)
      unlistenEnded = await events.on<string>('stream_ended', (payload) => {
        setStreamEnded(payload);
      });

      // Listen for stream error events (crash/unexpected exit)
      unlistenError = await events.on<StreamError>('stream_error', (payload) => {
        setStreamError(payload.groupId, payload.error);

        // Auto-retry if the backend says we can
        if (payload.canRetry) {
          handleAutoRetry(payload.groupId);
        }
      });

      // Listen for reconnecting events (backend-initiated retry in progress)
      unlistenReconnecting = await events.on<StreamReconnecting>('stream_reconnecting', (payload) => {
        toast.info(
          t('stream.reconnectingAttempt', 'Reconnecting... (attempt {{attempt}}/{{max}})', {
            attempt: payload.attempt,
            max: payload.maxAttempts,
          })
        );
      });
    };

    setupListeners();

    // Cleanup listeners on unmount
    return () => {
      if (unlistenStats) unlistenStats();
      if (unlistenEnded) unlistenEnded();
      if (unlistenError) unlistenError();
      if (unlistenReconnecting) unlistenReconnecting();
    };
  }, [updateStats, setStreamEnded, setStreamError, handleAutoRetry, t]);

  return null;
}

/**
 * Format seconds into HH:MM:SS
 */
export function formatUptime(seconds: number): string {
  const hrs = Math.floor(seconds / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  const secs = Math.floor(seconds % 60);

  const pad = (n: number) => n.toString().padStart(2, '0');

  if (hrs > 0) {
    return `${pad(hrs)}:${pad(mins)}:${pad(secs)}`;
  }
  return `${pad(mins)}:${pad(secs)}`;
}

/**
 * Format bytes to human readable size
 */
export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

/**
 * Format bitrate to human readable
 */
export function formatBitrate(kbps: number): string {
  if (kbps < 1000) return `${Math.round(kbps)} kbps`;
  return `${(kbps / 1000).toFixed(1)} Mbps`;
}
