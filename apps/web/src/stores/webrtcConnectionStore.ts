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
 *
 * Performance optimizations:
 * - Connection pooling: Reuses RTCPeerConnection objects to reduce connection setup time
 * - 100ms ICE timeout: Local STUN servers respond in <10ms, so 100ms is sufficient
 */

import { create } from 'zustand';
import { subscribeWithSelector } from 'zustand/middleware';
import { api } from '@/lib/backend';
import { isGo2rtcAvailable } from '@/lib/go2rtcStatus';

export type WebRTCStatus = 'idle' | 'loading' | 'connecting' | 'playing' | 'error' | 'unavailable';

/**
 * RTCPeerConnection pool for reusing connections
 * Reduces connection setup time by keeping pre-configured connections ready
 * Increased from 4 to 8 for multi-source scenarios (Studio Mode, Multiview)
 */
const MAX_POOL_SIZE = 8;
const connectionPool: RTCPeerConnection[] = [];

const rtcConfig: RTCConfiguration = {
  iceServers: [{ urls: 'stun:stun.l.google.com:19302' }],
};

/**
 * Get a connection from the pool or create a new one
 */
function getPooledConnection(): RTCPeerConnection {
  const pooled = connectionPool.pop();
  if (pooled && pooled.connectionState !== 'closed') {
    return pooled;
  }
  return new RTCPeerConnection(rtcConfig);
}

/**
 * Return a connection to the pool for reuse, or close it if pool is full
 */
function releaseConnection(pc: RTCPeerConnection): void {
  // Only pool connections that are still usable
  if (pc.connectionState === 'closed' || pc.connectionState === 'failed') {
    return;
  }

  // Reset connection state for reuse
  try {
    // Remove all tracks
    pc.getSenders().forEach((sender) => {
      try {
        pc.removeTrack(sender);
      } catch {
        // Ignore errors during cleanup
      }
    });

    // If pool has room, add to pool; otherwise close
    if (connectionPool.length < MAX_POOL_SIZE) {
      connectionPool.push(pc);
    } else {
      pc.close();
    }
  } catch {
    // If cleanup fails, just close the connection
    pc.close();
  }
}

/**
 * Pre-warm the connection pool with ready-to-use connections
 * Pre-configures transceivers to save 20-30ms during connection setup
 */
export function preWarmConnectionPool(count: number = 4): void {
  const toAdd = Math.min(count, MAX_POOL_SIZE - connectionPool.length);
  for (let i = 0; i < toAdd; i++) {
    const pc = new RTCPeerConnection(rtcConfig);
    // Pre-add transceivers to save 20-30ms during connection
    pc.addTransceiver('video', { direction: 'recvonly' });
    pc.addTransceiver('audio', { direction: 'recvonly' });
    connectionPool.push(pc);
  }
}

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
 * Uses connection pooling for faster connection setup
 */
async function connectWHEP(
  whepUrl: string,
  signal: AbortSignal
): Promise<{ pc: RTCPeerConnection; stream: MediaStream }> {
  // Get connection from pool or create new one
  const pc = getPooledConnection();

  // Clean up on abort - return to pool instead of closing
  signal.addEventListener('abort', () => {
    releaseConnection(pc);
  });

  // Create a promise that resolves when we get a stream
  const streamPromise = new Promise<MediaStream>((resolve) => {
    pc.ontrack = (event) => {
      if (event.streams[0]) {
        resolve(event.streams[0]);
      }
    };
  });

  // Create offer - check for existing transceivers from pool pre-warming
  // Per MDN: use receiver.track.kind to identify transceiver type
  // https://developer.mozilla.org/en-US/docs/Web/API/RTCRtpTransceiver
  const transceivers = pc.getTransceivers();
  const hasVideo = transceivers.some(t => t.receiver.track?.kind === 'video');
  const hasAudio = transceivers.some(t => t.receiver.track?.kind === 'audio');

  if (!hasVideo) {
    pc.addTransceiver('video', { direction: 'recvonly' });
  }
  if (!hasAudio) {
    pc.addTransceiver('audio', { direction: 'recvonly' });
  }

  const offer = await pc.createOffer();
  await pc.setLocalDescription(offer);

  // Wait for ICE gathering to complete (or timeout)
  // 50ms is sufficient for local STUN servers which respond in <10ms
  // Early completion detection allows immediate continuation when candidates are ready
  if (pc.iceGatheringState !== 'complete') {
    await new Promise<void>((resolve) => {
      let resolved = false;
      const complete = () => {
        if (!resolved && pc.iceGatheringState === 'complete') {
          resolved = true;
          pc.removeEventListener('icegatheringstatechange', complete);
          resolve();
        }
      };
      pc.addEventListener('icegatheringstatechange', complete);
      // 50ms fallback - local STUN typically responds in <10ms
      setTimeout(() => {
        if (!resolved) {
          resolved = true;
          pc.removeEventListener('icegatheringstatechange', complete);
          resolve();
        }
      }, 50);
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
        // Use cached go2rtc availability check for faster startup
        // The cache is pre-warmed by useBackendConnection hook
        if (conn.abortController?.signal.aborted) return;

        const available = await isGo2rtcAvailable();

        if (conn.abortController?.signal.aborted) return;

        if (!available) {
          console.log('[WebRTCStore] go2rtc not available for:', sourceId);
          set({
            connections: {
              ...get().connections,
              [sourceId]: {
                ...conn,
                status: 'unavailable',
                error: 'go2rtc WebRTC server is not running',
                isConnecting: false,
              },
            },
          });
          return;
        }

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

      // Release the peer connection back to pool for reuse
      // (more efficient than closing and creating new connections)
      if (conn.pc) {
        releaseConnection(conn.pc);
      }

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
