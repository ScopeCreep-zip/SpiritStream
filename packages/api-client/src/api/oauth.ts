import type { OAuthAccountStatus } from '@spiritstream/types';
import { fetchTypedJson } from './_internal';

/** Per-provider "real credentials present in this build/env" flags. */
export interface OAuthConfiguredFlags {
  twitchConfigured: boolean;
  youtubeConfigured: boolean;
  kickConfigured: boolean;
  facebookConfigured: boolean;
  trovoConfigured: boolean;
}

/**
 * `startFlow` response — the BACKEND chooses the grant per provider:
 * - `flow: 'redirect'` → loopback authorization-code; `authUrl` etc.
 *   present, `browserOpened: false` means show a copy-link affordance.
 * - `flow: 'device'`   → RFC 8628 device code (Twitch's mandated
 *   desktop sign-in); render `userCode` + `verificationUri` and wait
 *   for the `oauth_complete` event.
 */
export interface OAuthFlowStarted {
  flow: 'redirect' | 'device';
  authUrl?: string;
  callbackPort?: number;
  state?: string;
  browserOpened?: boolean;
  userCode?: string;
  verificationUri?: string;
  expiresIn?: number;
  interval?: number;
}

export const oauth = {
  isConfigured: (provider: string) =>
    fetchTypedJson<{ configured: boolean }>(
      'GET',
      `/api/v1/oauth/${encodeURIComponent(provider)}/configured`
    ).then((r) => r.configured),
  startFlow: (provider: string) =>
    fetchTypedJson<OAuthFlowStarted>('POST', `/api/v1/oauth/${encodeURIComponent(provider)}/flow`),
  completeFlow: (provider: string, code: string, state: string) =>
    fetchTypedJson<{
      provider: string;
      userId: string;
      username: string;
      displayName: string;
    }>('POST', `/api/v1/oauth/${encodeURIComponent(provider)}/complete`, undefined, {
      code,
      state,
    }),
  getAccount: (provider: string) =>
    fetchTypedJson<OAuthAccountStatus>(
      'GET',
      `/api/v1/oauth/${encodeURIComponent(provider)}/account`
    ),
  disconnect: async (provider: string) => {
    await fetchTypedJson<Record<string, never>>(
      'DELETE',
      `/api/v1/oauth/${encodeURIComponent(provider)}/account`
    );
  },
  forget: async (provider: string) => {
    await fetchTypedJson<Record<string, never>>(
      'POST',
      `/api/v1/oauth/${encodeURIComponent(provider)}/forget`
    );
  },
  refreshToken: (provider: string, refreshToken: string) =>
    fetchTypedJson<{
      accessToken: string;
      refreshToken?: string;
      expiresIn?: number;
    }>('POST', `/api/v1/oauth/${encodeURIComponent(provider)}/refresh`, undefined, {
      refreshToken,
    }),
  getConfig: () => fetchTypedJson<OAuthConfiguredFlags>('GET', '/api/v1/oauth/config'),
  setConfig: async (config: {
    twitchClientId?: string;
    twitchClientSecret?: string;
    youtubeClientId?: string;
    youtubeClientSecret?: string;
    kickClientId?: string;
    kickClientSecret?: string;
    facebookClientId?: string;
    facebookClientSecret?: string;
    trovoClientId?: string;
    trovoClientSecret?: string;
  }) => {
    await fetchTypedJson<Record<string, never>>('PUT', '/api/v1/oauth/config', undefined, config);
  },
};
