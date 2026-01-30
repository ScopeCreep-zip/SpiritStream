/**
 * useSharedWebRTCPreview hook
 * Provides shared WebRTC video streaming via go2rtc with connection deduplication
 *
 * When the same sourceId is requested by multiple components, they share a single
 * WebRTC connection. The connection is only closed when all consumers unmount.
 *
 * This significantly reduces resource usage when:
 * - The same source appears in multiple places (e.g., thumbnail + canvas)
 * - Multiple components render the same source
 */

import { useRef, useState, useEffect, useCallback } from 'react';
import { api } from '@/lib/backend';
import type { WebRTCStatus } from './useWebRTCPreview';

interface SharedConnection {
  refCount: number;
  pc: RTCPeerConnection | null;
  ws: WebSocket | null;
  stream: MediaStream | null;
  status: WebRTCStatus;
  error?: string;
  subscribers: Set<(status: WebRTCStatus, error?: string) => void>;
  abortController: AbortController | null;
  isConnecting: boolean;
}

// Global registry of active connections by sourceId
const connectionRegistry = new Map<string, SharedConnection>();

// Page visibility state (shared across all hooks)
let isPageVisible = !document.hidden;
const visibilitySubscribers = new Set<(visible: boolean) => void>();

// Set up global visibility listener once
if (typeof document !== 'undefined') {
  document.addEventListener('visibilitychange', () => {
    isPageVisible = !document.hidden;
    visibilitySubscribers.forEach((cb) => cb(isPageVisible));
  });
}

/**
 * Connect to a WHEP endpoint for WebRTC streaming
 */
async function connectWHEP(
  whepUrl: string,
  signal: AbortSignal
): Promise<{ pc: RTCPeerConnection; stream: MediaStream }> {
  const pc = new RTCPeerConnection({
    iceServers: [{ urls: 'stun:stun.l.google.com:19302' }],
  });

  // Clean up on abort
  signal.addEventListener('abort', () => {
    pc.close();
  });

  // Create a promise that resolves when we get a stream
  const streamPromise = new Promise<MediaStream>((resolve) => {
    pc.ontrack = (event) => {
      if (event.streams[0]) {
        resolve(event.streams[0]);
      }
    };
  });

  // Create offer
  pc.addTransceiver('video', { direction: 'recvonly' });
  pc.addTransceiver('audio', { direction: 'recvonly' });

  const offer = await pc.createOffer();
  await pc.setLocalDescription(offer);

  // Wait for ICE gathering to complete (or timeout)
  if (pc.iceGatheringState !== 'complete') {
    await new Promise<void>((resolve) => {
      const checkState = () => {
        if (pc.iceGatheringState === 'complete') {
          pc.removeEventListener('icegatheringstatechange', checkState);
          resolve();
        }
      };
      pc.addEventListener('icegatheringstatechange', checkState);
      setTimeout(resolve, 500);
    });
  }

  if (signal.aborted) {
    pc.close();
    throw new DOMException('Aborted', 'AbortError');
  }

  // Send offer to WHEP endpoint
  const response = await fetch(whepUrl, {
    method: 'POST',
    headers: { 'Content-Type': 'application/sdp' },
    body: pc.localDescription?.sdp,
    signal,
  });

  if (!response.ok) {
    pc.close();
    throw new Error(`WHEP request failed: ${response.status}`);
  }

  const answerSdp = await response.text();
  await pc.setRemoteDescription({ type: 'answer', sdp: answerSdp });

  // Wait for stream with timeout
  const stream = await Promise.race([
    streamPromise,
    new Promise<MediaStream>((_, reject) =>
      setTimeout(() => reject(new Error('Stream timeout')), 10000)
    ),
  ]);

  return { pc, stream };
}

/**
 * Get or create a shared connection for a sourceId
 */
async function getOrCreateConnection(
  sourceId: string,
  refreshKey: string | undefined,
  onStatusChange: (status: WebRTCStatus, error?: string) => void
): Promise<SharedConnection> {
  const key = `${sourceId}:${refreshKey ?? ''}`;
  let conn = connectionRegistry.get(key);

  if (conn) {
    // Existing connection - increment ref count and subscribe
    conn.refCount++;
    conn.subscribers.add(onStatusChange);
    // Immediately notify of current status
    onStatusChange(conn.status, conn.error);
    return conn;
  }

  // Create new connection entry
  conn = {
    refCount: 1,
    pc: null,
    ws: null,
    stream: null,
    status: 'loading',
    error: undefined,
    subscribers: new Set([onStatusChange]),
    abortController: null,
    isConnecting: false,
  };
  connectionRegistry.set(key, conn);

  // Start connection process
  await startConnection(key, sourceId);

  return conn;
}

/**
 * Start the WebRTC connection for a shared connection entry
 */
async function startConnection(key: string, sourceId: string): Promise<void> {
  const conn = connectionRegistry.get(key);
  if (!conn || conn.isConnecting) return;

  conn.isConnecting = true;
  conn.abortController?.abort();
  conn.abortController = new AbortController();

  const notifySubscribers = (status: WebRTCStatus, error?: string) => {
    conn.status = status;
    conn.error = error;
    conn.subscribers.forEach((cb) => cb(status, error));
  };

  notifySubscribers('loading');

  try {
    // Check if go2rtc is available
    const available = await api.webrtc.isAvailable();

    if (conn.abortController.signal.aborted) return;

    if (!available) {
      notifySubscribers('unavailable', 'go2rtc WebRTC server is not running');
      conn.isConnecting = false;
      return;
    }

    // Start WebRTC stream for this source
    notifySubscribers('connecting');
    const info = await api.webrtc.start(sourceId);

    if (conn.abortController.signal.aborted) return;

    if (!info.available) {
      notifySubscribers('unavailable', 'WebRTC stream could not be started');
      conn.isConnecting = false;
      return;
    }

    // Try WHEP (WebRTC)
    if (info.whepUrl) {
      try {
        const { pc, stream } = await connectWHEP(info.whepUrl, conn.abortController.signal);
        conn.pc = pc;
        conn.stream = stream;
        notifySubscribers('playing');
        conn.isConnecting = false;
        return;
      } catch (e) {
        if (e instanceof DOMException && e.name === 'AbortError') {
          conn.isConnecting = false;
          return;
        }
        console.warn('Shared WHEP connection failed:', e);
      }
    }

    // All methods failed
    notifySubscribers('error', 'Could not establish WebRTC connection');
  } catch (e) {
    if (e instanceof DOMException && e.name === 'AbortError') {
      conn.isConnecting = false;
      return;
    }
    console.error('Shared WebRTC connection error:', e);
    notifySubscribers('error', e instanceof Error ? e.message : 'Connection failed');
  }

  conn.isConnecting = false;
}

/**
 * Release a reference to a shared connection
 */
function releaseConnection(
  sourceId: string,
  refreshKey: string | undefined,
  onStatusChange: (status: WebRTCStatus, error?: string) => void
): void {
  const key = `${sourceId}:${refreshKey ?? ''}`;
  const conn = connectionRegistry.get(key);
  if (!conn) return;

  conn.subscribers.delete(onStatusChange);
  conn.refCount--;

  if (conn.refCount <= 0) {
    // Last consumer - clean up connection
    conn.abortController?.abort();
    conn.pc?.close();
    conn.ws?.close();
    connectionRegistry.delete(key);

    // Stop on server
    api.webrtc.stop(sourceId).catch(() => {
      // Ignore errors on cleanup
    });
  }
}

export interface SharedWebRTCPreviewResult {
  status: WebRTCStatus;
  stream: MediaStream | null;
  error?: string;
  retry: () => void;
}

/**
 * Hook to use a shared WebRTC preview connection
 * Multiple components using the same sourceId will share a single connection
 */
export function useSharedWebRTCPreview(
  sourceId: string,
  refreshKey?: string | number
): SharedWebRTCPreviewResult {
  const [status, setStatus] = useState<WebRTCStatus>('loading');
  const [error, setError] = useState<string>();
  const [stream, setStream] = useState<MediaStream | null>(null);
  const [retryCount, setRetryCount] = useState(0);
  const [pageVisible, setPageVisible] = useState(isPageVisible);

  const refreshKeyStr = refreshKey?.toString();
  const mountedRef = useRef(true);

  const retry = useCallback(() => {
    setRetryCount((c) => c + 1);
  }, []);

  // Subscribe to page visibility changes
  useEffect(() => {
    const handler = (visible: boolean) => {
      if (mountedRef.current) {
        setPageVisible(visible);
      }
    };
    visibilitySubscribers.add(handler);
    return () => {
      visibilitySubscribers.delete(handler);
    };
  }, []);

  // Main connection effect
  useEffect(() => {
    mountedRef.current = true;

    // Skip if no sourceId
    if (!sourceId) {
      setStatus('unavailable');
      setError(undefined);
      setStream(null);
      return;
    }

    // Skip if page is hidden
    if (!pageVisible) {
      setStream(null);
      return;
    }

    const onStatusChange = (newStatus: WebRTCStatus, newError?: string) => {
      if (!mountedRef.current) return;
      setStatus(newStatus);
      setError(newError);

      // Get the stream from the connection if playing
      const key = `${sourceId}:${refreshKeyStr ?? ''}`;
      const conn = connectionRegistry.get(key);
      if (newStatus === 'playing' && conn?.stream) {
        setStream(conn.stream);
      } else {
        setStream(null);
      }
    };

    getOrCreateConnection(sourceId, refreshKeyStr, onStatusChange);

    return () => {
      mountedRef.current = false;
      releaseConnection(sourceId, refreshKeyStr, onStatusChange);
    };
  }, [sourceId, refreshKeyStr, retryCount, pageVisible]);

  return {
    status,
    stream,
    error,
    retry,
  };
}

/**
 * Get the current number of active shared connections (for debugging)
 */
export function getActiveConnectionCount(): number {
  return connectionRegistry.size;
}

/**
 * Get details about active connections (for debugging)
 */
export function getActiveConnections(): Array<{ sourceId: string; refCount: number; status: WebRTCStatus }> {
  return Array.from(connectionRegistry.entries()).map(([key, conn]) => ({
    sourceId: key.split(':')[0],
    refCount: conn.refCount,
    status: conn.status,
  }));
}
