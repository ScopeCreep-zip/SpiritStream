import { useEffect, useRef, useCallback } from 'react';
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

// Auto-connect retry settings
const INITIAL_RETRY_DELAY = 2000; // 2 seconds
const MAX_RETRY_DELAY = 30000; // 30 seconds
const RETRY_BACKOFF_MULTIPLIER = 1.5;

/**
 * Hook that listens for OBS WebSocket events and handles:
 * 1. Connection status updates to the OBS store
 * 2. Stream state changes that may trigger SpiritStream start/stop
 *    (when direction is obs-to-spiritstream or bidirectional)
 * 3. Auto-connect on startup and reconnection on disconnect
 */
export function useObsEvents() {
  const {
    updateFromEvent,
    config,
    loadConfig,
    triggeredByUs,
    setTriggeredByUs,
    connectionStatus,
    connect,
  } = useObsStore();
  const { startAllGroups, stopAllGroups, isStreaming, activeGroups } = useStreamStore();
  const { current: currentProfile } = useProfileStore();

  // Track previous stream status to detect changes
  const prevStreamStatus = useRef<ObsStreamStatus>('unknown');

  // Auto-connect state
  const autoConnectAttempted = useRef(false);
  const retryTimeoutRef = useRef<number | null>(null);
  const currentRetryDelay = useRef(INITIAL_RETRY_DELAY);
  const manuallyDisconnected = useRef(false);
  const connectRef = useRef(connect);

  // Keep connect ref up to date
  useEffect(() => {
    connectRef.current = connect;
  }, [connect]);

  // Load OBS config on mount if not already loaded
  useEffect(() => {
    if (!config) {
      loadConfig();
    }
  }, [config, loadConfig]);

  // Schedule a retry with exponential backoff
  const scheduleRetry = useCallback(() => {
    if (manuallyDisconnected.current) {
      return;
    }

    if (retryTimeoutRef.current) {
      window.clearTimeout(retryTimeoutRef.current);
    }

    console.log(`[useObsEvents] Scheduling retry in ${currentRetryDelay.current}ms`);

    retryTimeoutRef.current = window.setTimeout(async () => {
      retryTimeoutRef.current = null;

      if (manuallyDisconnected.current) {
        console.log('[useObsEvents] Skipping auto-connect - manually disconnected');
        return;
      }

      try {
        console.log('[useObsEvents] Attempting OBS auto-connect...');
        await connectRef.current(false); // false = not manual, this is auto-connect
        // Reset retry delay on successful connection
        currentRetryDelay.current = INITIAL_RETRY_DELAY;
      } catch (error) {
        console.log('[useObsEvents] Auto-connect failed, will retry:', error);
        scheduleRetry();
      }
    }, currentRetryDelay.current);

    // Increase delay for next retry (exponential backoff)
    currentRetryDelay.current = Math.min(
      currentRetryDelay.current * RETRY_BACKOFF_MULTIPLIER,
      MAX_RETRY_DELAY
    );
  }, []);

  // Attempt to connect to OBS (isManual=false for auto-connect)
  const attemptConnect = useCallback(async () => {
    if (manuallyDisconnected.current) {
      console.log('[useObsEvents] Skipping auto-connect - manually disconnected');
      return;
    }

    try {
      console.log('[useObsEvents] Attempting OBS auto-connect...');
      await connectRef.current(false); // false = not manual, this is auto-connect
      // Reset retry delay on successful connection
      currentRetryDelay.current = INITIAL_RETRY_DELAY;
    } catch (error) {
      console.log('[useObsEvents] Auto-connect failed, will retry:', error);
      scheduleRetry();
    }
  }, [scheduleRetry]);

  // Auto-connect on startup if enabled
  useEffect(() => {
    if (!config || autoConnectAttempted.current) {
      return;
    }

    if (config.autoConnect && connectionStatus === 'disconnected') {
      autoConnectAttempted.current = true;
      // Small delay to let the app fully initialize
      const timeout = window.setTimeout(() => {
        attemptConnect();
      }, 1000);

      return () => window.clearTimeout(timeout);
    }
  }, [config, connectionStatus, attemptConnect]);

  // Handle disconnection - retry if auto-connect is enabled
  useEffect(() => {
    if (!config?.autoConnect) {
      return;
    }

    // If we were connected and now disconnected (not manually), schedule retry
    if (connectionStatus === 'disconnected' && !manuallyDisconnected.current) {
      scheduleRetry();
    }

    // If we're connected, clear any pending retry
    if (connectionStatus === 'connected' && retryTimeoutRef.current) {
      window.clearTimeout(retryTimeoutRef.current);
      retryTimeoutRef.current = null;
    }
  }, [connectionStatus, config?.autoConnect, scheduleRetry]);

  // Cleanup retry timeout on unmount
  useEffect(() => {
    return () => {
      if (retryTimeoutRef.current) {
        window.clearTimeout(retryTimeoutRef.current);
      }
    };
  }, []);

  // Track manual disconnection to prevent auto-reconnect
  useEffect(() => {
    const handleManualDisconnect = () => {
      manuallyDisconnected.current = true;
      if (retryTimeoutRef.current) {
        window.clearTimeout(retryTimeoutRef.current);
        retryTimeoutRef.current = null;
      }
    };

    const handleManualConnect = () => {
      manuallyDisconnected.current = false;
    };

    // These events are triggered by the obsStore when user clicks connect/disconnect
    window.addEventListener('obs:manual-disconnect', handleManualDisconnect);
    window.addEventListener('obs:manual-connect', handleManualConnect);

    return () => {
      window.removeEventListener('obs:manual-disconnect', handleManualDisconnect);
      window.removeEventListener('obs:manual-connect', handleManualConnect);
    };
  }, []);

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
