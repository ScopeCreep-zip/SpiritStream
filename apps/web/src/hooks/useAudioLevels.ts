/**
 * useAudioLevels Hook
 * Listens for audio_levels WebSocket events from the backend
 */
import { useState, useEffect } from 'react';
import { events } from '@/lib/backend/events';
import { useConnectionStore } from '@/stores/connectionStore';

export interface AudioLevel {
  rms: number;      // 0-1 RMS level
  peak: number;     // 0-1 peak level
  clipping: boolean; // Whether clipping was detected
}

export interface AudioLevelsData {
  tracks: Record<string, AudioLevel>;
  master: AudioLevel;
}

interface UseAudioLevelsResult {
  levels: AudioLevelsData | null;
  isConnected: boolean;
}

/**
 * Hook to subscribe to real-time audio levels from the backend
 */
export function useAudioLevels(): UseAudioLevelsResult {
  const [levels, setLevels] = useState<AudioLevelsData | null>(null);
  const [hasReceivedData, setHasReceivedData] = useState(false);
  const connectionStatus = useConnectionStore((s) => s.status);

  const isConnected = connectionStatus === 'connected' && hasReceivedData;

  useEffect(() => {
    let mounted = true;
    let unsubscribe: (() => void) | null = null;

    // Handler for audio_levels events
    const handleAudioLevels = (payload: AudioLevelsData) => {
      if (mounted) {
        setLevels(payload);
        setHasReceivedData(true);
      }
    };

    // Subscribe to audio_levels events
    events.on<AudioLevelsData>('audio_levels', handleAudioLevels).then((unsub) => {
      if (mounted) {
        unsubscribe = unsub;
      } else {
        // Component unmounted before subscription completed
        unsub();
      }
    });

    return () => {
      mounted = false;
      if (unsubscribe) {
        unsubscribe();
      }
    };
  }, []);

  // Reset when disconnected
  useEffect(() => {
    if (connectionStatus !== 'connected') {
      setLevels(null);
      setHasReceivedData(false);
    }
  }, [connectionStatus]);

  return { levels, isConnected };
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
