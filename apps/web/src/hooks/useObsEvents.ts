import { useEffect, useRef } from 'react';
import { events } from '@/lib/backend';
import { useObsStore } from '@/stores/obsStore';
import { useStreamStore } from '@/stores/streamStore';
import { useProfileStore } from '@/stores/profileStore';
import type { ObsConnectionStatus, ObsStreamStatus, ObsIntegrationDirection } from '@/types/api';

interface ObsStatusEvent {
  status: 'connecting' | 'connected' | 'disconnected' | 'error';
  host?: string;
  port?: number;
  obsVersion?: string;
  websocketVersion?: string;
  streamStatus?: ObsStreamStatus;
  error?: string;
}

interface ObsStreamStateEvent {
  status: ObsStreamStatus;
  active: boolean;
}

/**
 * Hook that listens for OBS WebSocket events and handles:
 * 1. Connection status updates to the OBS store
 * 2. Stream state changes that may trigger SpiritStream start/stop
 *    (when direction is obs-to-spiritstream or bidirectional)
 */
export function useObsEvents() {
  const { updateFromEvent, config, loadConfig, triggeredByUs, setTriggeredByUs } = useObsStore();
  const { startAllGroups, stopAllGroups, isStreaming, activeGroups } = useStreamStore();
  const { current: currentProfile } = useProfileStore();

  // Track previous stream status to detect changes
  const prevStreamStatus = useRef<ObsStreamStatus>('unknown');

  // Load OBS config on mount if not already loaded
  useEffect(() => {
    if (!config) {
      loadConfig();
    }
  }, [config, loadConfig]);

  useEffect(() => {
    let unlistenStatus: (() => void) | null = null;
    let unlistenStreamState: (() => void) | null = null;

    const setupListeners = async () => {
      // Listen for OBS connection status changes
      unlistenStatus = await events.on<ObsStatusEvent>('obs://status', (payload) => {
        console.log('[useObsEvents] Received obs://status:', payload);

        const connectionStatus: ObsConnectionStatus =
          payload.status === 'connecting' ? 'connecting' :
          payload.status === 'connected' ? 'connected' :
          payload.status === 'error' ? 'error' : 'disconnected';

        updateFromEvent({
          connectionStatus,
          obsVersion: payload.obsVersion ?? undefined,
          websocketVersion: payload.websocketVersion ?? undefined,
          errorMessage: payload.error ?? undefined,
          streamStatus: payload.streamStatus,
        });
      });

      // Listen for OBS stream state changes
      unlistenStreamState = await events.on<ObsStreamStateEvent>('obs://stream_state', (payload) => {
        console.log('[useObsEvents] Received obs://stream_state:', payload);

        const newStatus = payload.status;
        const wasActive = prevStreamStatus.current === 'active';
        const isNowActive = newStatus === 'active';
        const isNowInactive = newStatus === 'inactive';

        // Update store
        updateFromEvent({ streamStatus: newStatus });

        // Skip if this change was triggered by us (SpiritStream -> OBS)
        if (triggeredByUs) {
          console.log('[useObsEvents] Ignoring state change triggered by SpiritStream');
          setTriggeredByUs(false);
          prevStreamStatus.current = newStatus;
          return;
        }

        // Check if we should trigger SpiritStream
        const direction: ObsIntegrationDirection = config?.direction ?? 'disabled';
        const shouldObsTrigger = direction === 'obs-to-spiritstream' || direction === 'bidirectional';

        if (!shouldObsTrigger) {
          prevStreamStatus.current = newStatus;
          return;
        }

        // OBS started streaming -> Start SpiritStream
        if (!wasActive && isNowActive) {
          console.log('[useObsEvents] OBS started streaming, triggering SpiritStream');

          if (!currentProfile) {
            console.warn('[useObsEvents] No profile loaded, cannot start SpiritStream');
            prevStreamStatus.current = newStatus;
            return;
          }

          if (isStreaming || activeGroups.size > 0) {
            console.log('[useObsEvents] SpiritStream already streaming, skipping trigger');
            prevStreamStatus.current = newStatus;
            return;
          }

          // Start SpiritStream with current profile
          const eligibleGroups = currentProfile.outputGroups.filter(
            (g) => g.streamTargets.length > 0
          );

          if (eligibleGroups.length === 0) {
            console.warn('[useObsEvents] No eligible output groups to stream');
            prevStreamStatus.current = newStatus;
            return;
          }

          // Build incoming URL from structured input
          const { bindAddress, port, application } = currentProfile.input;
          const incomingUrl = `rtmp://${bindAddress}:${port}/${application}`;

          startAllGroups(eligibleGroups, incomingUrl).catch((error) => {
            console.error('[useObsEvents] Failed to start SpiritStream:', error);
          });
        }

        // OBS stopped streaming -> Stop SpiritStream
        if (wasActive && isNowInactive) {
          console.log('[useObsEvents] OBS stopped streaming, triggering SpiritStream stop');

          if (!isStreaming && activeGroups.size === 0) {
            console.log('[useObsEvents] SpiritStream not streaming, skipping stop trigger');
            prevStreamStatus.current = newStatus;
            return;
          }

          stopAllGroups().catch((error) => {
            console.error('[useObsEvents] Failed to stop SpiritStream:', error);
          });
        }

        prevStreamStatus.current = newStatus;
      });
    };

    setupListeners();

    return () => {
      if (unlistenStatus) unlistenStatus();
      if (unlistenStreamState) unlistenStreamState();
    };
  }, [config, currentProfile, isStreaming, activeGroups, updateFromEvent, startAllGroups, stopAllGroups, triggeredByUs, setTriggeredByUs]);
}
