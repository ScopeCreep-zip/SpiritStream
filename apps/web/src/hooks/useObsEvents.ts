import { useEffect } from 'react';
import { events } from '@spiritstream/api-client';
import { logger } from '@/lib/logger';
import { useObsStore } from '@/stores/obsStore';
import { useStreamStore } from '@/stores/streamStore';
import type { ObsConnectionStatus, ObsStreamStatus } from '@spiritstream/types';

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
  /** Backend-owned loop-prevention flag (see `ObsWebSocketHandler::triggered_by_us`). */
  triggeredByUs?: boolean;
}

/** Normalise the wire-format `status` string to the strict
 * `ObsConnectionStatus` union. Anything we don't recognise collapses to
 * `disconnected` so a broken backend ships a closed-looking widget
 * rather than a stuck-spinner. */
function toConnectionStatus(status: ObsStatusEvent['status']): ObsConnectionStatus {
  switch (status) {
    case 'connecting':
      return 'connecting';
    case 'connected':
      return 'connected';
    case 'error':
      return 'error';
    default:
      return 'disconnected';
  }
}

/**
 * Mirror backend OBS events into the OBS store for display.
 *
 * The cascade decisions ("should we start SpiritStream when OBS
 * starts?", "should we trigger OBS when SpiritStream starts?",
 * auto-connect retry policy) all live in `crates/core/src/services/obs_websocket.rs`
 * — this hook is a thin event-to-store wrapper. The frontend
 * answers "how should this look?", never "should this action happen?".
 */
export function useObsEvents() {
  const { updateFromEvent, config, loadConfig } = useObsStore();
  const setIsStreaming = useStreamStore((s) => s.setIsStreaming);

  // Load OBS config on mount if not already loaded
  useEffect(() => {
    if (!config) {
      loadConfig();
    }
  }, [config, loadConfig]);

  useEffect(() => {
    // Track unmount across the async setup window. Pre-fix, cleanup ran
    // synchronously while `setupListeners()` was still awaiting — the
    // unlisten vars were still `null` so cleanup did nothing, and the
    // listeners that eventually registered leaked forever. Now: if the
    // hook unmounts before a listener finishes registering, we abort
    // each unlisten the moment it resolves so nothing escapes.
    let cancelled = false;
    let unlistenStatus: (() => void) | null = null;
    let unlistenStreamState: (() => void) | null = null;
    let unlistenStartedByObs: (() => void) | null = null;
    let unlistenStoppedByObs: (() => void) | null = null;

    const setupListeners = async (): Promise<void> => {
      const status = await events.on<ObsStatusEvent>('obs://status', (payload) => {
        logger.debug('[useObsEvents] obs://status', payload);
        const connectionStatus = toConnectionStatus(payload.status);
        updateFromEvent({
          connectionStatus,
          obsVersion: payload.obsVersion ?? undefined,
          websocketVersion: payload.websocketVersion ?? undefined,
          errorMessage: payload.error ?? undefined,
          streamStatus: payload.streamStatus,
        });
      });
      if (cancelled) {
        status();
        return;
      }
      unlistenStatus = status;

      const streamState = await events.on<ObsStreamStateEvent>('obs://stream_state', (payload) => {
        logger.debug('[useObsEvents] obs://stream_state', payload);
        updateFromEvent({ streamStatus: payload.status });
      });
      if (cancelled) {
        streamState();
        return;
      }
      unlistenStreamState = streamState;

      // Core ran the OBS→SpiritStream cascade — backend has actually
      // started/stopped ffmpeg. Mirror the resulting streaming state
      // into the stream store so the StatusStrip + pipeline rows
      // reflect reality. Pre-this wiring the UI showed "offline"
      // while ffmpeg was running after an OBS-driven start — the
      // listener only logged the event without updating state.
      const startedByObs = await events.on('stream_started_by_obs', (payload) => {
        logger.info('[useObsEvents] stream_started_by_obs', payload);
        setIsStreaming(true);
      });
      if (cancelled) {
        startedByObs();
        return;
      }
      unlistenStartedByObs = startedByObs;

      const stoppedByObs = await events.on('stream_stopped_by_obs', (payload) => {
        logger.info('[useObsEvents] stream_stopped_by_obs', payload);
        setIsStreaming(false);
      });
      if (cancelled) {
        stoppedByObs();
        return;
      }
      unlistenStoppedByObs = stoppedByObs;
    };

    setupListeners().catch((error) => {
      logger.error('[useObsEvents] failed to register listeners:', error);
    });

    return () => {
      cancelled = true;
      if (unlistenStatus) unlistenStatus();
      if (unlistenStreamState) unlistenStreamState();
      if (unlistenStartedByObs) unlistenStartedByObs();
      if (unlistenStoppedByObs) unlistenStoppedByObs();
    };
  }, [updateFromEvent, setIsStreaming]);
}
