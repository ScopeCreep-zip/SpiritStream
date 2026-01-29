/**
 * useWebRTCPreview hook
 * Provides WebRTC video streaming via go2rtc
 *
 * Flow:
 * 1. Check if go2rtc is available
 * 2. If available, start WebRTC stream for source
 * 3. Connect via WHEP (WebRTC-HTTP Egress Protocol)
 * 4. If WHEP fails, try MSE via WebSocket
 * 5. If all fail, show error state with retry option
 */

import { useRef, useState, useEffect, useCallback } from 'react';
import { api } from '@/lib/backend';

export type WebRTCStatus = 'loading' | 'connecting' | 'playing' | 'error' | 'unavailable';

export interface WebRTCPreviewResult {
  status: WebRTCStatus;
  videoRef: React.RefObject<HTMLVideoElement | null>;
  error?: string;
  retry: () => void;
}

/**
 * Connect to a WHEP endpoint for WebRTC streaming
 */
async function connectWHEP(
  whepUrl: string,
  video: HTMLVideoElement,
  signal: AbortSignal
): Promise<RTCPeerConnection> {
  const pc = new RTCPeerConnection({
    iceServers: [{ urls: 'stun:stun.l.google.com:19302' }],
  });

  // Clean up on abort
  signal.addEventListener('abort', () => {
    pc.close();
  });

  // Handle incoming tracks
  pc.ontrack = (event) => {
    if (event.streams[0]) {
      video.srcObject = event.streams[0];
    }
  };

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
      // Timeout after 3 seconds
      setTimeout(resolve, 3000);
    });
  }

  // Check if aborted during ICE gathering
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

  return pc;
}

/**
 * Connect to go2rtc via MSE over WebSocket (secondary method if WHEP fails)
 */
async function connectMSE(
  wsUrl: string,
  video: HTMLVideoElement,
  signal: AbortSignal
): Promise<WebSocket> {
  return new Promise((resolve, reject) => {
    if (signal.aborted) {
      reject(new DOMException('Aborted', 'AbortError'));
      return;
    }

    const ws = new WebSocket(wsUrl);
    let mediaSource: MediaSource | null = null;
    let sourceBuffer: SourceBuffer | null = null;

    const cleanup = () => {
      ws.close();
      if (mediaSource && video.src) {
        URL.revokeObjectURL(video.src);
      }
    };

    signal.addEventListener('abort', () => {
      cleanup();
      reject(new DOMException('Aborted', 'AbortError'));
    });

    ws.onopen = () => {
      mediaSource = new MediaSource();
      video.src = URL.createObjectURL(mediaSource);

      mediaSource.onsourceopen = () => {
        try {
          sourceBuffer = mediaSource!.addSourceBuffer('video/mp4; codecs="avc1.42E01E"');
          resolve(ws);
        } catch (e) {
          cleanup();
          reject(e);
        }
      };
    };

    ws.onmessage = (event) => {
      if (sourceBuffer && !sourceBuffer.updating && event.data instanceof ArrayBuffer) {
        try {
          sourceBuffer.appendBuffer(event.data);
        } catch {
          // Buffer full or other error - skip frame
        }
      }
    };

    ws.onerror = () => {
      cleanup();
      reject(new Error('WebSocket connection failed'));
    };

    ws.onclose = () => {
      if (mediaSource && video.src) {
        URL.revokeObjectURL(video.src);
      }
    };
  });
}

export function useWebRTCPreview(sourceId: string): WebRTCPreviewResult {
  const videoRef = useRef<HTMLVideoElement>(null);
  const [status, setStatus] = useState<WebRTCStatus>('loading');
  const [error, setError] = useState<string>();
  const [retryCount, setRetryCount] = useState(0);

  const pcRef = useRef<RTCPeerConnection | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const abortControllerRef = useRef<AbortController | null>(null);
  const mountedRef = useRef(true);

  const retry = useCallback(() => {
    setRetryCount((c) => c + 1);
  }, []);

  // Main connection effect
  useEffect(() => {
    mountedRef.current = true;

    // Cleanup function for resources
    const cleanupResources = () => {
      if (pcRef.current) {
        pcRef.current.close();
        pcRef.current = null;
      }
      if (wsRef.current) {
        wsRef.current.close();
        wsRef.current = null;
      }
      if (videoRef.current) {
        videoRef.current.srcObject = null;
        if (videoRef.current.src) {
          URL.revokeObjectURL(videoRef.current.src);
          videoRef.current.src = '';
        }
      }
    };

    const connect = async () => {
      // Abort any previous connection
      abortControllerRef.current?.abort();
      cleanupResources();

      if (!sourceId) {
        setStatus('error');
        setError('No source ID provided');
        return;
      }

      const abortController = new AbortController();
      abortControllerRef.current = abortController;

      setStatus('loading');
      setError(undefined);

      try {
        // Check if go2rtc is available
        const available = await api.webrtc.isAvailable();

        if (!mountedRef.current || abortController.signal.aborted) return;

        if (!available) {
          setStatus('unavailable');
          setError('go2rtc WebRTC server is not running');
          return;
        }

        // Start WebRTC stream for this source
        setStatus('connecting');
        const info = await api.webrtc.start(sourceId);

        if (!mountedRef.current || abortController.signal.aborted) return;

        if (!info.available) {
          setStatus('unavailable');
          setError('WebRTC stream could not be started');
          return;
        }

        const video = videoRef.current;
        if (!video) {
          throw new Error('Video element not available');
        }

        // Try WHEP first (WebRTC)
        if (info.whepUrl) {
          try {
            pcRef.current = await connectWHEP(info.whepUrl, video, abortController.signal);
            if (mountedRef.current && !abortController.signal.aborted) {
              setStatus('playing');
            }
            return;
          } catch (e) {
            if (e instanceof DOMException && e.name === 'AbortError') {
              return; // Intentional abort, don't try MSE
            }
            console.warn('WHEP connection failed, trying MSE:', e);
          }
        }

        // Try MSE over WebSocket
        if (info.wsUrl) {
          try {
            wsRef.current = await connectMSE(info.wsUrl, video, abortController.signal);
            if (mountedRef.current && !abortController.signal.aborted) {
              setStatus('playing');
            }
            return;
          } catch (e) {
            if (e instanceof DOMException && e.name === 'AbortError') {
              return; // Intentional abort
            }
            console.warn('MSE connection failed:', e);
          }
        }

        // All methods failed
        if (mountedRef.current && !abortController.signal.aborted) {
          setStatus('error');
          setError('Could not establish WebRTC connection');
        }
      } catch (e) {
        if (e instanceof DOMException && e.name === 'AbortError') {
          return; // Intentional abort
        }

        if (mountedRef.current) {
          console.error('WebRTC preview error:', e);
          setError(e instanceof Error ? e.message : 'Connection failed');
          setStatus('error');
        }
      }
    };

    connect();

    return () => {
      mountedRef.current = false;
      abortControllerRef.current?.abort();
      cleanupResources();

      // Stop the WebRTC stream on the server
      if (sourceId) {
        api.webrtc.stop(sourceId).catch(() => {
          // Ignore errors on cleanup
        });
      }
    };
  }, [sourceId, retryCount]); // Only reconnect when sourceId changes or retry is triggered

  return {
    status,
    videoRef,
    error,
    retry,
  };
}
