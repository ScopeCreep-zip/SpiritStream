import { useEffect } from 'react';
import { events } from '@spiritstream/api-client';
import { logger } from '@/lib/logger';
import { useObsStore } from '@/stores/obsStore';
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

  // Load OBS config on mount if not already loaded
  useEffect(() => {
    if (!config) {
      loadConfig();
    }
  }, [config, loadConfig]);

  useEffect(() => {
    let unlistenStatus: (() => void) | null = null;
    let unlistenStreamState: (() => void) | null = null;
    let unlistenStartedByObs: (() => void) | null = null;
    let unlistenStoppedByObs: (() => void) | null = null;

    const setupListeners = async () => {
      unlistenStatus = await events.on<ObsStatusEvent>('obs://status', (payload) => {
        logger.debug('[useObsEvents] obs://status', payload);
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

      unlistenStreamState = await events.on<ObsStreamStateEvent>(
        'obs://stream_state',
        (payload) => {
          logger.debug('[useObsEvents] obs://stream_state', payload);
          updateFromEvent({ streamStatus: payload.status });
        },
      );

      // Informational: core ran the OBS→SpiritStream cascade. UI may
      // surface a brief toast / indicator. No decision-making here.
      unlistenStartedByObs = await events.on('stream_started_by_obs', (payload) => {
        logger.info('[useObsEvents] stream_started_by_obs', payload);
      });
      unlistenStoppedByObs = await events.on('stream_stopped_by_obs', (payload) => {
        logger.info('[useObsEvents] stream_stopped_by_obs', payload);
      });
    };

    setupListeners();

    return () => {
      if (unlistenStatus) unlistenStatus();
      if (unlistenStreamState) unlistenStreamState();
      if (unlistenStartedByObs) unlistenStartedByObs();
      if (unlistenStoppedByObs) unlistenStoppedByObs();
    };
  }, [updateFromEvent]);
}
