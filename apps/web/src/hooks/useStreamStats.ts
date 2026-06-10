import { useEffect } from 'react';
import { events } from '@spiritstream/api-client';
import type {
  StreamStats,
  StreamErrorEvent,
  StreamRetryAttemptEvent,
  StreamRetryExhaustedEvent,
} from '@spiritstream/types';
import { useStreamStore } from '@/stores/streamStore';
import { logger } from '@/lib/logger';
import { toast } from '@/hooks/useToast';
import { useTranslation } from 'react-i18next';

/**
 * Hook to listen to real-time stream statistics from the Rust backend.
 *
 * The retry POLICY and the retry TRIGGER both live on the backend.
 * The server subscribes to its own `stream_error` events via
 * `start_auto_retry_task` and runs `retry_group` with the configured backoff.
 * This hook just renders `stream_retry_attempt` toasts.
 */
export function useStreamStats() {
  const { t } = useTranslation();
  const { updateStats, setStreamEnded, setStreamError } = useStreamStore();
  const isStreaming = useStreamStore((s) => s.isStreaming);
  const incrementUptime = useStreamStore((s) => s.incrementUptime);

  // Smooth-tick the displayed uptime so the StatusStrip clock advances
  // between authoritative `stream_stats` events. FFmpeg emits stats
  // roughly once per second under normal load, but bursty ingest /
  // codec-stall conditions can stretch the interval — without this
  // tick the clock visibly freezes. `updateStats` overwrites uptime
  // with the authoritative ffmpeg `time` value the next time stats
  // arrive, so the tick can only ever be optimistic, never wrong.
  useEffect(() => {
    if (!isStreaming) return;
    const handle = window.setInterval(() => {
      incrementUptime();
    }, 1000);
    return () => window.clearInterval(handle);
  }, [isStreaming, incrementUptime]);

  // Set up event listeners
  useEffect(() => {
    let cancelled = false;
    let unlistenStats: (() => void) | null = null;
    let unlistenEnded: (() => void) | null = null;
    let unlistenError: (() => void) | null = null;
    let unlistenRetry: (() => void) | null = null;
    let unlistenExhausted: (() => void) | null = null;

    const setupListeners = async () => {
      // Listen for stream stats updates
      const stats = await events.on<StreamStats>('stream_stats', (payload) => {
        updateStats(payload.groupId, payload);
      });
      if (cancelled) {
        stats();
        return;
      }
      unlistenStats = stats;

      // Listen for stream ended events (clean exit)
      const ended = await events.on<string>('stream_ended', (payload) => {
        setStreamEnded(payload);
      });
      if (cancelled) {
        ended();
        return;
      }
      unlistenEnded = ended;

      // Listen for stream error events (crash/unexpected exit). Backend
      // owns the retry trigger (`start_auto_retry_task`); the store flips
      // status, and a toast tells the user WHY — the error string used to
      // land in an `error` field nothing rendered.
      const errored = await events.on<StreamErrorEvent>('stream_error', (payload) => {
        setStreamError(payload.groupId);
        toast.error(
          t('streams.streamError', 'Stream error: {{error}}', { error: payload.error }) +
            (payload.suggestion ? ` ${payload.suggestion}` : '')
        );
      });
      if (cancelled) {
        errored();
        return;
      }
      unlistenError = errored;

      // Listen for retry-attempt events (backend-initiated retry in progress)
      const retry = await events.on<StreamRetryAttemptEvent>(
        'stream_retry_attempt',
        (payload) => {
          toast.info(
            t('streams.reconnectingAttempt', 'Reconnecting... (attempt {{attempt}}/{{max}})', {
              attempt: payload.attempt,
              max: payload.maxAttempts,
            })
          );
        }
      );
      if (cancelled) {
        retry();
        return;
      }
      unlistenRetry = retry;

      // Listen for terminal retry exhaustion — backend gave up. Flip the
      // group to an error state so the UI no longer shows "active".
      const exhausted = await events.on<StreamRetryExhaustedEvent>(
        'stream_retry_exhausted',
        (payload) => {
          setStreamError(payload.groupId);
          toast.error(
            t('streams.retryExhausted', 'Stream failed after {{max}} retries — restart manually.', {
              max: payload.maxAttempts,
            })
          );
        }
      );
      if (cancelled) {
        exhausted();
        return;
      }
      unlistenExhausted = exhausted;
    };

    setupListeners().catch((error) => {
      logger.error('[useStreamStats] failed to register listeners:', error);
    });

    // Cleanup listeners on unmount
    return () => {
      cancelled = true;
      if (unlistenStats) unlistenStats();
      if (unlistenEnded) unlistenEnded();
      if (unlistenError) unlistenError();
      if (unlistenRetry) unlistenRetry();
      if (unlistenExhausted) unlistenExhausted();
    };
  }, [updateStats, setStreamEnded, setStreamError, t]);

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
