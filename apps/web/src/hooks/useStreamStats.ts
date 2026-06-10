import { useEffect } from 'react';
import { events } from '@spiritstream/api-client';
import type { StreamStats } from '@spiritstream/types';
import { useStreamStore } from '@/stores/streamStore';
import { toast } from '@/hooks/useToast';
import { useTranslation } from 'react-i18next';

// L3: pre-fix this file shadowed every backend stream-event type with
// a hand-maintained inline `interface` (StreamStats, StreamError,
// StreamReconnecting, StreamRetryExhausted). They drifted from the
// ts-rs-generated source of truth (`packages/types/src/generated/`).
// Backend `StreamStats` is now imported above. For the other three —
// the backend emits them as `serde_json::Value` payloads on the event
// bus, so the local shape is the wire-side schema. Each one is named
// `_StreamEventPayload` so it's clear they describe the bus payload
// and not the (non-existent) backend type.

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
 * Terminal event: the backend gave up after max_retries. UI should treat the
 * group as failed (not in retrying limbo) and let the user manually restart.
 */
export interface StreamRetryExhausted {
  groupId: string;
  maxAttempts: number;
}

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
    let unlistenStats: (() => void) | null = null;
    let unlistenEnded: (() => void) | null = null;
    let unlistenError: (() => void) | null = null;
    let unlistenRetry: (() => void) | null = null;
    let unlistenExhausted: (() => void) | null = null;

    const setupListeners = async () => {
      // Listen for stream stats updates
      unlistenStats = await events.on<StreamStats>('stream_stats', (payload) => {
        updateStats(payload.groupId, payload);
      });

      // Listen for stream ended events (clean exit)
      unlistenEnded = await events.on<string>('stream_ended', (payload) => {
        setStreamEnded(payload);
      });

      // Listen for stream error events (crash/unexpected exit). Backend
      // owns the retry trigger (`start_auto_retry_task`); we just surface
      // the error in the store so the UI can render an indicator.
      unlistenError = await events.on<StreamError>('stream_error', (payload) => {
        setStreamError(payload.groupId, payload.error);
      });

      // Listen for retry-attempt events (backend-initiated retry in progress)
      unlistenRetry = await events.on<StreamReconnecting>('stream_retry_attempt', (payload) => {
        toast.info(
          t('streams.reconnectingAttempt', 'Reconnecting... (attempt {{attempt}}/{{max}})', {
            attempt: payload.attempt,
            max: payload.maxAttempts,
          })
        );
      });

      // Listen for terminal retry exhaustion — backend gave up. Flip the
      // group to an error state so the UI no longer shows "active".
      unlistenExhausted = await events.on<StreamRetryExhausted>(
        'stream_retry_exhausted',
        (payload) => {
          setStreamError(
            payload.groupId,
            t('streams.retryExhausted', 'Stream failed after {{max}} retries — restart manually.', {
              max: payload.maxAttempts,
            })
          );
        }
      );
    };

    setupListeners();

    // Cleanup listeners on unmount
    return () => {
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
