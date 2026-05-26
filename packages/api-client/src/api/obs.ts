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
    await fetchTypedJson<Record<string, never>>('PUT', '/api/v1/obs/config', undefined, config);
  },
  connect: async () => {
    await fetchTypedJson<Record<string, never>>('POST', '/api/v1/obs/connection');
  },
  disconnect: async () => {
    await fetchTypedJson<Record<string, never>>('DELETE', '/api/v1/obs/connection');
  },
  startStream: async () => {
    await fetchTypedJson<Record<string, never>>('POST', '/api/v1/obs/stream');
  },
  stopStream: async () => {
    await fetchTypedJson<Record<string, never>>('DELETE', '/api/v1/obs/stream');
  },
  isConnected: () =>
    fetchTypedJson<{ connected: boolean }>('GET', '/api/v1/obs/connection').then((r) => r.connected),
};
