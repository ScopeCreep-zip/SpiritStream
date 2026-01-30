/**
 * useWebRTCStream hook
 *
 * Simple hook that subscribes to a WebRTC stream from the persistent connection store.
 * Unlike the old useSharedWebRTCPreview, this hook does NOT manage connection lifecycle.
 * Connections are managed by WebRTCConnectionManager at the app level.
 *
 * Usage:
 * ```tsx
 * const { stream, status, error, retry } = useWebRTCStream(sourceId);
 * ```
 */

import { useCallback } from 'react';
import { useWebRTCConnectionStore, type WebRTCStatus } from '@/stores/webrtcConnectionStore';

export interface WebRTCStreamResult {
  /** The MediaStream if connected, null otherwise */
  stream: MediaStream | null;
  /** Current connection status */
  status: WebRTCStatus;
  /** Error message if status is 'error' */
  error?: string;
  /** Retry the connection */
  retry: () => void;
}

/**
 * Hook to subscribe to a WebRTC stream from the persistent connection store.
 *
 * This hook only provides read access to the stream. Connection lifecycle is
 * managed by WebRTCConnectionManager based on profile sources.
 *
 * @param sourceId - The source ID to get the stream for
 * @returns Stream, status, error, and retry function
 */
export function useWebRTCStream(sourceId: string): WebRTCStreamResult {
  // Subscribe to specific connection state using selector for optimal re-renders
  const stream = useWebRTCConnectionStore(
    useCallback((state) => state.connections[sourceId]?.stream ?? null, [sourceId])
  );

  const status = useWebRTCConnectionStore(
    useCallback((state) => state.connections[sourceId]?.status ?? 'idle', [sourceId])
  );

  const error = useWebRTCConnectionStore(
    useCallback((state) => state.connections[sourceId]?.error, [sourceId])
  );

  const retryConnection = useWebRTCConnectionStore((state) => state.retryConnection);

  const retry = useCallback(() => {
    if (sourceId) {
      retryConnection(sourceId);
    }
  }, [sourceId, retryConnection]);

  return {
    stream,
    status,
    error,
    retry,
  };
}

// Re-export the status type for convenience
export type { WebRTCStatus };
