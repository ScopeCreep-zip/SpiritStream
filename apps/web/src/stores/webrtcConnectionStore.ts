/**
 * WebRTC Connection Store
 *
 * Manages persistent WebRTC connections that survive page visibility changes
 * and component unmounts. Connections are tied to the profile's source lifecycle,
 * not the React component lifecycle.
 *
 * Key design decisions:
 * 1. No visibility-based cleanup - connections stay alive when page loses focus
 * 2. No refCount-based cleanup - connections stay alive when components unmount
 * 3. Lifecycle tied to profile sources - start when source is added, stop when removed
 * 4. Components just subscribe - simple hook to get stream, no connection management
 */

import { create } from 'zustand';
import { subscribeWithSelector } from 'zustand/middleware';
import { api } from '@/lib/backend';
import { events } from '@/lib/backend/httpEvents';

export type WebRTCStatus = 'idle' | 'loading' | 'connecting' | 'playing' | 'error' | 'unavailable';

interface WebRTCConnection {
  pc: RTCPeerConnection | null;
  stream: MediaStream | null;
  status: WebRTCStatus;
  error?: string;
  abortController: AbortController | null;
  isConnecting: boolean;
  /** Number of times this connection has been retried */
  retryCount: number;
}

interface WebRTCConnectionState {
  /** Record of sourceId -> connection (using Record instead of Map for React compatibility) */
  connections: Record<string, WebRTCConnection>;

  /** Start a WebRTC connection for a source */
  startConnection: (sourceId: string) => Promise<void>;

  /** Stop a WebRTC connection for a source */
  stopConnection: (sourceId: string) => Promise<void>;

  /** Stop all WebRTC connections */
  stopAllConnections: () => Promise<void>;

  /** Retry a failed connection */
  retryConnection: (sourceId: string) => Promise<void>;

  /** Get stream for a source (returns null if not connected) */
  getStream: (sourceId: string) => MediaStream | null;

  /** Get status for a source */
  getStatus: (sourceId: string) => WebRTCStatus;

  /** Get error for a source */
  getError: (sourceId: string) => string | undefined;
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
  // 200ms is sufficient for local STUN - faster than default 500ms
  if (pc.iceGatheringState !== 'complete') {
    await new Promise<void>((resolve) => {
      const checkState = () => {
        if (pc.iceGatheringState === 'complete') {
          pc.removeEventListener('icegatheringstatechange', checkState);
          resolve();
        }
      };
      pc.addEventListener('icegatheringstatechange', checkState);
      setTimeout(resolve, 200);
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
 * Create the default connection state
 */
function createDefaultConnection(): WebRTCConnection {
  return {
    pc: null,
    stream: null,
    status: 'idle',
    error: undefined,
    abortController: null,
    isConnecting: false,
    retryCount: 0,
  };
}

export const useWebRTCConnectionStore = create<WebRTCConnectionState>()(
  subscribeWithSelector((set, get) => ({
    connections: {},

    startConnection: async (sourceId: string) => {
      const { connections } = get();

      // Check if already connected or connecting
      const existing = connections[sourceId];
      if (existing && (existing.status === 'playing' || existing.isConnecting)) {
        console.log('[WebRTCStore] Already connected/connecting:', sourceId, existing.status);
        return;
      }

      console.log('[WebRTCStore] Starting connection for:', sourceId);

      // Create or update connection entry
      const conn: WebRTCConnection = {
        ...createDefaultConnection(),
        status: 'loading',
        isConnecting: true,
        abortController: new AbortController(),
        retryCount: existing?.retryCount ?? 0,
      };

      // Abort any existing connection attempt
      existing?.abortController?.abort();

      set({ connections: { ...connections, [sourceId]: conn } });

      try {
        if (conn.abortController?.signal.aborted) return;

        // Update status to connecting
        {
          const current = get().connections[sourceId];
          if (current) {
            set({
              connections: {
                ...get().connections,
                [sourceId]: { ...current, status: 'connecting' },
              },
            });
          }
        }

        // Start WebRTC stream for this source
        const info = await api.webrtc.start(sourceId);

        if (conn.abortController?.signal.aborted) return;

        if (!info.available) {
          console.log('[WebRTCStore] WebRTC stream not available for:', sourceId);
          set({
            connections: {
              ...get().connections,
              [sourceId]: {
                ...conn,
                status: 'unavailable',
                error: 'WebRTC stream could not be started',
                isConnecting: false,
              },
            },
          });
          return;
        }

        console.log('[WebRTCStore] Got stream info for:', sourceId, info);

        // Try WHEP (WebRTC)
        if (info.whepUrl) {
          try {
            const { pc, stream } = await connectWHEP(
              info.whepUrl,
              conn.abortController!.signal
            );

            console.log('[WebRTCStore] WHEP connected for:', sourceId);
            set({
              connections: {
                ...get().connections,
                [sourceId]: {
                  ...conn,
                  pc,
                  stream,
                  status: 'playing',
                  error: undefined,
                  isConnecting: false,
                  retryCount: 0, // Reset retry count on success
                },
              },
            });
            return;
          } catch (e) {
            if (e instanceof DOMException && e.name === 'AbortError') {
              return;
            }
            console.warn(`[WebRTCStore] WHEP connection failed for ${sourceId}:`, e);
          }
        }

        // All methods failed
        set({
          connections: {
            ...get().connections,
            [sourceId]: {
              ...conn,
              status: 'error',
              error: 'Could not establish WebRTC connection',
              isConnecting: false,
            },
          },
        });
      } catch (e) {
        if (e instanceof DOMException && e.name === 'AbortError') {
          return;
        }
        console.error(`[WebRTCStore] Connection error for ${sourceId}:`, e);
        set({
          connections: {
            ...get().connections,
            [sourceId]: {
              ...conn,
              status: 'error',
              error: e instanceof Error ? e.message : 'Connection failed',
              isConnecting: false,
            },
          },
        });
      }
    },

    stopConnection: async (sourceId: string) => {
      const { connections } = get();
      const conn = connections[sourceId];

      if (!conn) return;

      // Abort any ongoing connection attempt
      conn.abortController?.abort();

      // Close the peer connection
      conn.pc?.close();

      // Remove from registry
      const { [sourceId]: _, ...rest } = connections;
      set({ connections: rest });

      // Stop on server
      try {
        await api.webrtc.stop(sourceId);
      } catch {
        // Ignore errors on cleanup
      }
    },

    stopAllConnections: async () => {
      const { connections, stopConnection } = get();

      // Stop all connections
      const stopPromises = Object.keys(connections).map((sourceId) =>
        stopConnection(sourceId)
      );

      await Promise.all(stopPromises);
    },

    retryConnection: async (sourceId: string) => {
      const { connections, startConnection } = get();
      const conn = connections[sourceId];

      if (conn) {
        // Increment retry count
        set({
          connections: {
            ...connections,
            [sourceId]: {
              ...conn,
              retryCount: conn.retryCount + 1,
              status: 'idle',
              error: undefined,
            },
          },
        });
      }

      await startConnection(sourceId);
    },

    getStream: (sourceId: string) => {
      return get().connections[sourceId]?.stream ?? null;
    },

    getStatus: (sourceId: string) => {
      return get().connections[sourceId]?.status ?? 'idle';
    },

    getError: (sourceId: string) => {
      return get().connections[sourceId]?.error;
    },
  }))
);

// Auto-retry connections stuck in 'unavailable' when go2rtc becomes available.
// This handles the race condition where connections are attempted before go2rtc
// has lazy-started. The server emits 'go2rtc_status' after first successful start.
if (typeof window !== 'undefined') {
  events.on<{ available?: boolean }>('go2rtc_status', (payload) => {
    if (!payload?.available) return;

    const { connections, retryConnection } = useWebRTCConnectionStore.getState();
    const unavailable = Object.entries(connections)
      .filter(([, conn]) => conn.status === 'unavailable')
      .map(([sourceId]) => sourceId);

    if (unavailable.length > 0) {
      console.log('[WebRTCStore] go2rtc now available, retrying', unavailable.length, 'connections');
      for (const sourceId of unavailable) {
        retryConnection(sourceId);
      }
    }
  }).catch(() => {
    // WebSocket not ready yet — fine, no connections to retry anyway
  });
}

/**
 * Get the current number of active connections (for debugging)
 */
export function getActiveConnectionCount(): number {
  return Object.keys(useWebRTCConnectionStore.getState().connections).length;
}

/**
 * Get details about active connections (for debugging)
 */
export function getActiveConnections(): Array<{
  sourceId: string;
  status: WebRTCStatus;
  retryCount: number;
}> {
  const { connections } = useWebRTCConnectionStore.getState();
  return Object.entries(connections).map(([sourceId, conn]) => ({
    sourceId,
    status: conn.status,
    retryCount: conn.retryCount,
  }));
}
