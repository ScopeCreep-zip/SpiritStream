/**
 * WebRTCConnectionManager
 *
 * App-level component that manages WebRTC connection lifecycle based on profile sources.
 * This component renders at the App level (never unmounts) and syncs connections with
 * the current profile's sources.
 *
 * Key behaviors:
 * 1. Starts connections for sources that need WebRTC when profile loads
 * 2. Stops connections when sources are removed from profile
 * 3. Stops all connections when profile is unloaded
 * 4. Does NOT stop connections on page visibility change or navigation
 */

import { useEffect, useRef } from 'react';
import { useProfileStore } from '@/stores/profileStore';
import { useWebRTCConnectionStore } from '@/stores/webrtcConnectionStore';
import type { WebRTCStatus } from '@/stores/webrtcConnectionStore';
import { sourceNeedsWebRTC } from '@/lib/mediaTypes';
import type { Source } from '@/types/profile';

/**
 * Wait for a source's WebRTC connection to reach a terminal state.
 * Prevents thundering herd by ensuring each source's capture pipeline
 * (scap + FFmpeg + go2rtc) stabilizes before starting the next.
 */
function waitForConnectionStable(sourceId: string, timeoutMs: number): Promise<void> {
  return new Promise((resolve) => {
    const timeout = setTimeout(() => {
      unsub();
      resolve();
    }, timeoutMs);

    const TERMINAL_STATES: WebRTCStatus[] = ['playing', 'error', 'unavailable'];

    // Check if already in terminal state
    const currentStatus = useWebRTCConnectionStore.getState().connections[sourceId]?.status;
    if (currentStatus && TERMINAL_STATES.includes(currentStatus)) {
      clearTimeout(timeout);
      resolve();
      return;
    }

    const unsub = useWebRTCConnectionStore.subscribe(
      (state) => state.connections[sourceId]?.status,
      (status) => {
        if (status && TERMINAL_STATES.includes(status)) {
          clearTimeout(timeout);
          unsub();
          // Brief settle time for system resources to free up
          setTimeout(resolve, 300);
        }
      }
    );
  });
}

// Selector that returns a stable string of source IDs (JSON for comparison)
function selectWebRTCSourceIds(state: { current: { sources: Source[] } | null }): string {
  if (!state.current?.sources) return '';
  return state.current.sources
    .filter((source) =>
      sourceNeedsWebRTC({
        type: source.type,
        filePath: 'filePath' in source ? source.filePath : undefined,
      })
    )
    .map((s) => s.id)
    .join(',');
}

export function WebRTCConnectionManager() {
  // Use a selector that returns a primitive string (stable reference)
  const webrtcSourceIdsStr = useProfileStore(selectWebRTCSourceIds);
  const currentProfileName = useProfileStore((state) => state.current?.name);

  // Get store actions (these are stable references)
  const startConnection = useWebRTCConnectionStore((state) => state.startConnection);
  const stopConnection = useWebRTCConnectionStore((state) => state.stopConnection);
  const stopAllConnections = useWebRTCConnectionStore((state) => state.stopAllConnections);

  // Track the previous profile name to detect profile switches
  const prevProfileRef = useRef<string | undefined>(undefined);

  // Track which connections we've started to avoid reading from store in effect
  const activeConnectionsRef = useRef<Set<string>>(new Set());

  // Sync connections with sources
  useEffect(() => {
    // Parse source IDs from string inside effect
    const webrtcSourceIds = webrtcSourceIdsStr ? webrtcSourceIdsStr.split(',') : [];

    // If profile changed, stop all existing connections first
    if (prevProfileRef.current !== currentProfileName) {
      if (prevProfileRef.current !== undefined) {
        // Profile switched (not initial load) - stop all connections
        stopAllConnections();
        activeConnectionsRef.current.clear();
      }
      prevProfileRef.current = currentProfileName;
    }

    // If no profile loaded, ensure all connections are stopped
    if (!currentProfileName) {
      if (activeConnectionsRef.current.size > 0) {
        stopAllConnections();
        activeConnectionsRef.current.clear();
      }
      return;
    }

    const webrtcSourceIdSet = new Set(webrtcSourceIds);

    // Collect new sources that need connections
    const newSources = webrtcSourceIds.filter((id) => !activeConnectionsRef.current.has(id));

    // Sequential connection startup — each source waits for the previous to stabilize.
    // Each connection triggers heavy backend work: native capture + FFmpeg H264 encoder + go2rtc.
    // Starting all at once exhausts HW encoder slots (3-4 on Apple Silicon) and causes
    // go2rtc WHEP 500 errors when the producer can't connect under load.
    let cancelled = false;
    if (newSources.length > 0) {
      const sequentialStart = async (): Promise<void> => {
        for (let i = 0; i < newSources.length; i++) {
          if (cancelled) return;
          const sourceId = newSources[i];
          console.log(
            `[WebRTCManager] Starting connection for: ${sourceId} (${i + 1}/${newSources.length})`
          );
          startConnection(sourceId);
          activeConnectionsRef.current.add(sourceId);

          // Wait for this connection to reach a terminal state before starting next.
          // Prevents resource contention from concurrent capture pipelines.
          // 15s timeout ensures we don't block forever if a source hangs.
          if (i < newSources.length - 1) {
            await waitForConnectionStable(sourceId, 15000);
          }
        }
      };
      sequentialStart();
    }

    // Stop connections for removed sources
    for (const sourceId of activeConnectionsRef.current) {
      if (!webrtcSourceIdSet.has(sourceId)) {
        stopConnection(sourceId);
        activeConnectionsRef.current.delete(sourceId);
      }
    }

    return () => {
      cancelled = true;
    };
  }, [webrtcSourceIdsStr, currentProfileName, startConnection, stopConnection, stopAllConnections]);

  // Cleanup on unmount (app closing)
  useEffect(() => {
    return () => {
      stopAllConnections();
    };
  }, [stopAllConnections]);

  // This component doesn't render anything
  return null;
}
