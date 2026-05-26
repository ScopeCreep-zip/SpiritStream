import type { OAuthAccountStatus, OAuthFlowResult } from '@spiritstream/types';
import { fetchTypedJson } from './_internal';

export const oauth = {
  isConfigured: (provider: string) =>
    fetchTypedJson<{ configured: boolean }>(
      'GET',
      `/api/v1/oauth/${encodeURIComponent(provider)}/configured`,
    ).then((r) => r.configured),
  startFlow: (provider: string) =>
    fetchTypedJson<OAuthFlowResult>(
      'POST',
      `/api/v1/oauth/${encodeURIComponent(provider)}/flow`,
    ),
  completeFlow: (provider: string, code: string, state: string) =>
    fetchTypedJson<{
      provider: string;
      userId: string;
      username: string;
      displayName: string;
    }>(
      'POST',
      `/api/v1/oauth/${encodeURIComponent(provider)}/complete`,
      undefined,
      { code, state },
    ),
  getAccount: (provider: string) =>
    fetchTypedJson<OAuthAccountStatus>(
      'GET',
      `/api/v1/oauth/${encodeURIComponent(provider)}/account`,
    ),
  disconnect: async (provider: string) => {
    await fetchTypedJson<Record<string, never>>(
      'DELETE',
      `/api/v1/oauth/${encodeURIComponent(provider)}/account`,
    );
  },
  forget: async (provider: string) => {
    await fetchTypedJson<Record<string, never>>(
      'POST',
      `/api/v1/oauth/${encodeURIComponent(provider)}/forget`,
    );
  },
  refreshToken: (provider: string, refreshToken: string) =>
    fetchTypedJson<{
      accessToken: string;
      refreshToken?: string;
      expiresIn?: number;
    }>(
      'POST',
      `/api/v1/oauth/${encodeURIComponent(provider)}/refresh`,
      undefined,
      { refreshToken },
    ),
  getConfig: () =>
    fetchTypedJson<{ twitchConfigured: boolean; youtubeConfigured: boolean }>(
      'GET',
      '/api/v1/oauth/config',
    ),
  setConfig: async (config: {
    twitchClientId?: string;
    twitchClientSecret?: string;
    youtubeClientId?: string;
    youtubeClientSecret?: string;
  }) => {
    await fetchTypedJson<Record<string, never>>('PUT', '/api/v1/oauth/config', undefined, config);
  },
};
