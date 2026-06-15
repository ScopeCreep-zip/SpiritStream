import type { ObsState } from '@spiritstream/types';
import {
  v1ObsStateProxy,
  v1ObsConnectProxy,
  v1ObsDisconnectProxy,
  v1ObsStartStreamProxy,
  v1ObsStopStreamProxy,
  v1ObsIsConnectedProxy,
} from '../generated';

// OBS settings are NOT read/written through the API. The active profile is the
// single source of truth: settings persist via the profile save endpoint, and
// the backend syncs its runtime handler from the profile on activation/save.
// This client only drives the runtime connection (connect/disconnect/stream).

export const obs = {
  getState: async (): Promise<ObsState> => {
    const { data } = await v1ObsStateProxy({ throwOnError: true });
    return data as ObsState;
  },
  connect: async (): Promise<void> => {
    await v1ObsConnectProxy({ throwOnError: true });
  },
  disconnect: async (): Promise<void> => {
    await v1ObsDisconnectProxy({ throwOnError: true });
  },
  startStream: async (): Promise<void> => {
    await v1ObsStartStreamProxy({ throwOnError: true });
  },
  stopStream: async (): Promise<void> => {
    await v1ObsStopStreamProxy({ throwOnError: true });
  },
  isConnected: async (): Promise<boolean> => {
    const { data } = await v1ObsIsConnectedProxy({ throwOnError: true });
    return data.connected;
  },
};
