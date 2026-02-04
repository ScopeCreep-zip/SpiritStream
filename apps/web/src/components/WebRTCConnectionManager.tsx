/**
 * WebRTCConnectionManager
 *
 * App-level component that manages WebRTC connection lifecycle based on profile sources.
 * This component renders at the App level (never unmounts) and syncs connections with
 * the current profile's sources.
 *
 * Key behaviors:
 * 1. Starts connections for sources that need WebRTC when profile loads (in parallel)
 * 2. Stops connections when sources are removed from profile
 * 3. Stops all connections when profile is unloaded
 * 4. Does NOT stop connections on page visibility change or navigation
 *
 * Performance optimizations:
 * - Parallel WebRTC startup: All new connections are started concurrently using Promise.all
 *   to reduce multi-source startup time from 2-10s to <1s
 */

import { useEffect, useRef } from 'react';
import { useProfileStore } from '@/stores/profileStore';
import { useWebRTCConnectionStore } from '@/stores/webrtcConnectionStore';
import { sourceNeedsWebRTC } from '@/lib/mediaTypes';
import type { Source } from '@/types/profile';

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
    const newSourceIds: string[] = [];
    for (const sourceId of webrtcSourceIds) {
      if (!activeConnectionsRef.current.has(sourceId)) {
        newSourceIds.push(sourceId);
        activeConnectionsRef.current.add(sourceId);
      }
    }

    // Start all new connections in parallel for faster multi-source startup
    // This reduces startup time from 2-10s (sequential) to <1s (parallel)
    if (newSourceIds.length > 0) {
      console.log('[WebRTCManager] Starting connections in parallel for:', newSourceIds);
      Promise.all(
        newSourceIds.map((sourceId) =>
          startConnection(sourceId).catch((e) =>
            console.warn(`[WebRTCManager] Connection failed for ${sourceId}:`, e)
          )
        )
      );
    }

    // Stop connections for removed sources
    for (const sourceId of activeConnectionsRef.current) {
      if (!webrtcSourceIdSet.has(sourceId)) {
        stopConnection(sourceId);
        activeConnectionsRef.current.delete(sourceId);
      }
    }
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
