import type { ObsConfig, ObsIntegrationDirection, ObsState } from '@spiritstream/types';
import { fetchTypedJson } from './_internal';

export const obs = {
  getState: () => fetchTypedJson<ObsState>('GET', '/api/v1/obs/state'),
  getConfig: () => fetchTypedJson<ObsConfig>('GET', '/api/v1/obs/config'),
  setConfig: async (config: {
    host: string;
    port: number;
    password?: string;
    useAuth: boolean;
    direction: ObsIntegrationDirection;
    autoConnect: boolean;
  }) => {
    await fetchTypedJson<unknown>('PUT', '/api/v1/obs/config', undefined, config);
  },
  connect: async () => {
    await fetchTypedJson<unknown>('POST', '/api/v1/obs/connection');
  },
  disconnect: async () => {
    await fetchTypedJson<unknown>('DELETE', '/api/v1/obs/connection');
  },
  startStream: async () => {
    await fetchTypedJson<unknown>('POST', '/api/v1/obs/stream');
  },
  stopStream: async () => {
    await fetchTypedJson<unknown>('DELETE', '/api/v1/obs/stream');
  },
  isConnected: () => fetchTypedJson<boolean>('GET', '/api/v1/obs/connection'),
};
