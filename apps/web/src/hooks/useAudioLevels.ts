/**
 * useAudioLevels Hook
 * Listens for audio_levels WebSocket events from the backend
 *
 * PERFORMANCE OPTIMIZATION:
 * Audio level data is routed to a pure JS store (audioLevelStore) that
 * bypasses React's render cycle. This eliminates ~30 re-renders per second.
 * Components read levels directly from the store in RAF loops.
 *
 * This hook only manages:
 * - Connection status (isConnected, isInitializing)
 * - Capture status per source
 * - Health status per source
 */
import { useState, useEffect, useCallback, useRef } from 'react';
import { events } from '@/lib/backend/events';
import { useConnectionStore } from '@/stores/connectionStore';
import type { AudioCaptureResult } from '@/lib/backend/httpApi';
import { api } from '@/lib/backend/httpApi';
import { updateLevels, resetLevels, type AudioLevelsData } from '@/lib/audio/audioLevelStore';

export interface AudioLevel {
  rms: number;      // 0-1 RMS level
  peak: number;     // 0-1 peak level
  peakDb: number;   // Peak level in dB for display
  clipping: boolean; // Whether clipping was detected
  // Stereo channel levels (optional)
  leftRms?: number;   // 0-1 left channel RMS
  leftPeak?: number;  // 0-1 left channel peak
  rightRms?: number;  // 0-1 right channel RMS
  rightPeak?: number; // 0-1 right channel peak
}

// Re-export AudioLevelsData for backwards compatibility
export type { AudioLevelsData } from '@/lib/audio/audioLevelStore';

/** Per-source capture status for display in the UI */
export type CaptureStatus = Record<string, AudioCaptureResult>;

/** Per-source health status (true = receiving data) */
export type SourceHealthStatus = Record<string, boolean>;

interface UseAudioLevelsResult {
  // NOTE: `levels` has been removed - components read directly from audioLevelStore
  // This eliminates ~30 React re-renders per second
  isConnected: boolean;
  /** True when backend is connected but waiting for first audio data */
  isInitializing: boolean;
  /** Capture status per source (set when setMonitorSources is called) */
  captureStatus: CaptureStatus;
  /** Update capture status from API response */
  setCaptureStatus: (status: CaptureStatus) => void;
  /** Health status per source (true = receiving data) */
  healthStatus: SourceHealthStatus;
}

/**
 * Hook to subscribe to real-time audio levels from the backend.
 *
 * PERFORMANCE: Audio level data is routed to a pure JS store that
 * bypasses React. Components read levels directly in RAF loops.
 * This hook only provides connection/capture/health status.
 */
export function useAudioLevels(): UseAudioLevelsResult {
  // NOTE: Removed setLevels state - data goes to pure JS store instead
  const [hasReceivedData, setHasReceivedData] = useState(false);
  const [captureStatus, setCaptureStatusInternal] = useState<CaptureStatus>({});
  const [healthStatus, setHealthStatus] = useState<SourceHealthStatus>({});
  const connectionStatus = useConnectionStore((s) => s.status);
  const healthPollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const isConnected = connectionStatus === 'connected' && hasReceivedData;
  // True when backend is connected but we haven't received audio data yet
  const isInitializing = connectionStatus === 'connected' && !hasReceivedData;

  // Expose setCaptureStatus for external callers (e.g., Stream.tsx after calling setMonitorSources)
  const setCaptureStatus = useCallback((status: CaptureStatus) => {
    setCaptureStatusInternal(status);
  }, []);

  // Poll health status periodically (every 5 seconds) when connected
  useEffect(() => {
    if (connectionStatus !== 'connected') {
      // Clear health polling when disconnected
      if (healthPollRef.current) {
        clearInterval(healthPollRef.current);
        healthPollRef.current = null;
      }
      setHealthStatus({});
      return;
    }

    // Start health polling
    const pollHealth = async () => {
      try {
        const result = await api.audio.getMonitorHealth();
        setHealthStatus(result.sources);
      } catch (err) {
        // Silently fail - health check is not critical
        console.debug('[useAudioLevels] Health check failed:', err);
      }
    };

    // Initial poll
    pollHealth();

    // Poll every 5 seconds
    healthPollRef.current = setInterval(pollHealth, 5000);

    return () => {
      if (healthPollRef.current) {
        clearInterval(healthPollRef.current);
        healthPollRef.current = null;
      }
    };
  }, [connectionStatus]);

  useEffect(() => {
    let mounted = true;
    let unsubscribe: (() => void) | null = null;
    let receivedCount = 0;

    console.log('[useAudioLevels] Subscribing to audio_levels events...');

    // Handler for audio_levels events
    // PERFORMANCE: Routes data to pure JS store, bypasses React state
    const handleAudioLevels = (payload: AudioLevelsData) => {
      if (mounted) {
        // Update pure JS store (no React re-render)
        updateLevels(payload);

        // Only update React state for connection status
        setHasReceivedData(true);

        // Log first few events and then periodically to confirm data is flowing
        receivedCount++;
        const trackCount = Object.keys(payload.tracks).length;
        if (receivedCount <= 3 || (receivedCount % 100 === 0 && trackCount > 0)) {
          console.log(
            `[useAudioLevels] Event #${receivedCount}: ${trackCount} tracks, master rms=${payload.master.rms.toFixed(4)}`
          );
          if (trackCount > 0 && receivedCount <= 3) {
            console.log('[useAudioLevels] Track IDs:', Object.keys(payload.tracks));
          }
        }
      }
    };

    // Subscribe to audio_levels events
    events
      .on<AudioLevelsData>('audio_levels', handleAudioLevels)
      .then((unsub) => {
        if (mounted) {
          unsubscribe = unsub;
          console.log('[useAudioLevels] ✓ Subscribed successfully');
        } else {
          // Component unmounted before subscription completed
          unsub();
        }
      })
      .catch((err) => {
        console.error('[useAudioLevels] ✗ Subscription failed:', err);
      });

    return () => {
      mounted = false;
      if (unsubscribe) {
        unsubscribe();
        console.log(`[useAudioLevels] Unsubscribed (received ${receivedCount} events)`);
      }
    };
  }, []);

  // Reset when disconnected
  useEffect(() => {
    if (connectionStatus !== 'connected') {
      // Reset pure JS store
      resetLevels();
      setHasReceivedData(false);
    }
  }, [connectionStatus]);

  // NOTE: `levels` removed from return - components read from audioLevelStore directly
  return { isConnected, isInitializing, captureStatus, setCaptureStatus, healthStatus };
}

/**
 * Convert dB value to linear (0-1) scale
 */
export function dbToLinear(db: number): number {
  return Math.pow(10, db / 20);
}

/**
 * Convert linear (0-1) value to dB
 */
export function linearToDb(linear: number): number {
  if (linear <= 0) return -Infinity;
  return 20 * Math.log10(linear);
}

/**
 * Get color for a given audio level
 */
export function getLevelColor(level: number): string {
  if (level > 0.9) return '#ef4444'; // Red - clipping danger
  if (level > 0.7) return '#f97316'; // Orange - warning
  if (level > 0.5) return '#eab308'; // Yellow - nominal high
  return '#22c55e'; // Green - safe
}
