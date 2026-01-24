/**
 * WebRTC Client for go2rtc
 *
 * Handles WebRTC connection to go2rtc server for low-latency video preview.
 * Uses WebSocket signaling to establish peer connection.
 *
 * @see https://github.com/AlexxIT/go2rtc
 */

export interface Go2rtcClientOptions {
  /** WebSocket signaling URL (e.g., ws://127.0.0.1:1984/api/ws?src=preview_xxx) */
  wsUrl: string;
  /** ICE servers for STUN/TURN */
  iceServers?: RTCIceServer[];
  /** Connection timeout in milliseconds */
  timeout?: number;
  /** Called when connection state changes */
  onConnectionStateChange?: (state: RTCPeerConnectionState) => void;
  /** Called on error */
  onError?: (error: Error) => void;
}

interface Go2rtcMessage {
  type: string;
  sdp?: string;
  candidate?: string;
  sdpMid?: string;
  sdpMLineIndex?: number;
}

/**
 * WebRTC client for connecting to go2rtc streams.
 *
 * Usage:
 * ```typescript
 * const client = new Go2rtcClient({
 *   wsUrl: 'ws://127.0.0.1:1984/api/ws?src=preview_camera_1',
 *   onConnectionStateChange: (state) => console.log('State:', state),
 * });
 *
 * const stream = await client.connect();
 * videoElement.srcObject = stream;
 *
 * // Later...
 * client.disconnect();
 * ```
 */
export class Go2rtcClient {
  private ws: WebSocket | null = null;
  private pc: RTCPeerConnection | null = null;
  private options: Required<Go2rtcClientOptions>;
  private connectionPromise: Promise<MediaStream> | null = null;
  private connectionResolve: ((stream: MediaStream) => void) | null = null;
  private connectionReject: ((error: Error) => void) | null = null;

  constructor(options: Go2rtcClientOptions) {
    this.options = {
      wsUrl: options.wsUrl,
      iceServers: options.iceServers ?? [
        { urls: 'stun:stun.l.google.com:19302' },
        { urls: 'stun:stun1.l.google.com:19302' },
      ],
      timeout: options.timeout ?? 15000,
      onConnectionStateChange: options.onConnectionStateChange ?? (() => {}),
      onError: options.onError ?? (() => {}),
    };
  }

  /**
   * Connect to the go2rtc stream.
   * Returns a MediaStream that can be assigned to a video element.
   */
  async connect(): Promise<MediaStream> {
    // Prevent multiple simultaneous connections
    if (this.connectionPromise) {
      return this.connectionPromise;
    }

    this.connectionPromise = new Promise<MediaStream>((resolve, reject) => {
      this.connectionResolve = resolve;
      this.connectionReject = reject;

      // Set connection timeout
      const timeoutId = setTimeout(() => {
        this.handleError(new Error('Connection timeout'));
      }, this.options.timeout);

      try {
        // Create peer connection
        this.pc = new RTCPeerConnection({
          iceServers: this.options.iceServers,
        });

        // Handle track event (when remote stream is received)
        this.pc.ontrack = (event) => {
          clearTimeout(timeoutId);
          if (event.streams && event.streams[0]) {
            this.connectionResolve?.(event.streams[0]);
          } else {
            // Create a new MediaStream with the track
            const stream = new MediaStream([event.track]);
            this.connectionResolve?.(stream);
          }
        };

        // Handle connection state changes
        this.pc.onconnectionstatechange = () => {
          const state = this.pc?.connectionState;
          if (state) {
            this.options.onConnectionStateChange(state);

            if (state === 'failed' || state === 'closed') {
              clearTimeout(timeoutId);
              this.handleError(new Error(`Connection ${state}`));
            }
          }
        };

        // Handle ICE candidates
        this.pc.onicecandidate = (event) => {
          if (event.candidate) {
            this.sendMessage({
              type: 'webrtc/candidate',
              candidate: event.candidate.candidate,
              sdpMid: event.candidate.sdpMid ?? undefined,
              sdpMLineIndex: event.candidate.sdpMLineIndex ?? undefined,
            });
          }
        };

        // Add transceivers for receiving audio and video
        this.pc.addTransceiver('video', { direction: 'recvonly' });
        // Optional: add audio transceiver
        // this.pc.addTransceiver('audio', { direction: 'recvonly' });

        // Connect WebSocket
        this.ws = new WebSocket(this.options.wsUrl);

        this.ws.onopen = () => {
          // Request WebRTC stream
          this.sendMessage({ type: 'webrtc' });
        };

        this.ws.onmessage = async (event) => {
          try {
            const msg: Go2rtcMessage = JSON.parse(event.data);
            await this.handleMessage(msg);
          } catch (e) {
            console.error('Failed to handle WebSocket message:', e);
          }
        };

        this.ws.onerror = () => {
          clearTimeout(timeoutId);
          this.handleError(new Error('WebSocket error'));
        };

        this.ws.onclose = () => {
          clearTimeout(timeoutId);
          // Only reject if not already resolved
          if (this.connectionReject) {
            this.handleError(new Error('WebSocket closed'));
          }
        };
      } catch (error) {
        clearTimeout(timeoutId);
        this.handleError(error instanceof Error ? error : new Error(String(error)));
      }
    });

    return this.connectionPromise;
  }

  /**
   * Disconnect from the stream and clean up resources.
   */
  disconnect(): void {
    // Close WebSocket
    if (this.ws) {
      this.ws.onclose = null; // Prevent error handling
      this.ws.close();
      this.ws = null;
    }

    // Close peer connection
    if (this.pc) {
      this.pc.ontrack = null;
      this.pc.onconnectionstatechange = null;
      this.pc.onicecandidate = null;
      this.pc.close();
      this.pc = null;
    }

    // Clear promise state
    this.connectionPromise = null;
    this.connectionResolve = null;
    this.connectionReject = null;
  }

  /**
   * Get current connection state.
   */
  get connectionState(): RTCPeerConnectionState | null {
    return this.pc?.connectionState ?? null;
  }

  /**
   * Check if connected.
   */
  get isConnected(): boolean {
    return this.pc?.connectionState === 'connected';
  }

  private sendMessage(msg: Go2rtcMessage): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(msg));
    }
  }

  private async handleMessage(msg: Go2rtcMessage): Promise<void> {
    if (!this.pc) return;

    switch (msg.type) {
      case 'webrtc/offer':
        // go2rtc sends offer, we respond with answer
        if (msg.sdp) {
          await this.pc.setRemoteDescription({
            type: 'offer',
            sdp: msg.sdp,
          });

          const answer = await this.pc.createAnswer();
          await this.pc.setLocalDescription(answer);

          this.sendMessage({
            type: 'webrtc/answer',
            sdp: answer.sdp,
          });
        }
        break;

      case 'webrtc/candidate':
        // Remote ICE candidate
        if (msg.candidate) {
          try {
            await this.pc.addIceCandidate({
              candidate: msg.candidate,
              sdpMid: msg.sdpMid ?? null,
              sdpMLineIndex: msg.sdpMLineIndex ?? null,
            });
          } catch (e) {
            // Ignore candidate errors (common during connection setup)
            console.debug('ICE candidate error:', e);
          }
        }
        break;

      default:
        console.debug('Unknown go2rtc message type:', msg.type);
    }
  }

  private handleError(error: Error): void {
    this.options.onError(error);
    this.connectionReject?.(error);
    this.connectionReject = null;
    this.connectionResolve = null;
    this.disconnect();
  }
}

/**
 * Create a Go2rtcClient with backend URL construction.
 *
 * @param streamId - The stream ID from the backend (e.g., 'preview_camera_1')
 * @param go2rtcPort - The go2rtc API port (default: 1984)
 * @param options - Additional client options
 */
export function createGo2rtcClient(
  streamId: string,
  go2rtcPort: number = 1984,
  options: Partial<Go2rtcClientOptions> = {}
): Go2rtcClient {
  const wsUrl = `ws://127.0.0.1:${go2rtcPort}/api/ws?src=${streamId}`;
  return new Go2rtcClient({
    wsUrl,
    ...options,
  });
}
