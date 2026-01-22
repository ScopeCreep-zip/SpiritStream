import { useEffect } from 'react';
import { events } from '@/lib/backend';
import { useStreamStore } from '@/stores/streamStore';

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
}

/**
 * Hook to listen to real-time stream statistics from the Rust backend
 */
export function useStreamStats() {
  const { updateStats, setStreamEnded, setStreamError } = useStreamStore();

  // Set up event listeners
  useEffect(() => {
    let unlistenStats: (() => void) | null = null;
    let unlistenEnded: (() => void) | null = null;
    let unlistenError: (() => void) | null = null;

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
      });
    };

    setupListeners();

    // Cleanup listeners on unmount
    return () => {
      if (unlistenStats) unlistenStats();
      if (unlistenEnded) unlistenEnded();
      if (unlistenError) unlistenError();
    };
  }, [updateStats, setStreamEnded, setStreamError]);

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
