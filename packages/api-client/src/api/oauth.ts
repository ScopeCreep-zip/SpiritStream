import type { OAuthAccountStatus } from '@spiritstream/types';
import {
  v1OauthIsConfiguredProxy,
  v1OauthStartFlowProxy,
  v1OauthCompleteFlowProxy,
  v1OauthGetAccountProxy,
  v1OauthDisconnectProxy,
  v1OauthForgetProxy,
  v1OauthRefreshTokenProxy,
  v1OauthGetConfigProxy,
  v1OauthSetProviderCredentialsProxy,
  v1OauthSetConfigProxy,
} from '../generated';

/** One value the user pastes/selects in a provider's dev console. */
export interface OAuthConsoleField {
  /** The console's own field label, e.g. "Name" or "OAuth Redirect URL". */
  label: string;
  /** The exact value to paste, or the option to choose. */
  value: string;
  /** `true` → show a copy button; `false` → it's a selection, nothing to copy. */
  copyable: boolean;
  /** One-line caveat shown under the field. */
  note?: string | null;
}

/** One numbered instruction; `url` is a direct deep-link to the page this
 *  step happens on (e.g. Google's separate Audience / Data Access pages),
 *  shown as a copy-the-link button since the webview can't open links. */
export interface OAuthStep {
  text: string;
  url?: string | null;
  /** Values to paste at this step, shown inline where the step asks for them. */
  fields: OAuthConsoleField[];
}

/** Pre-filled guided walkthrough for registering an app — all backend-computed. */
export interface OAuthProviderSetup {
  /** Numbered instructions shown before the fields (may be empty). */
  steps: OAuthStep[];
  /** The labeled values to paste/select into the console form, in order. */
  consoleFields: OAuthConsoleField[];
}

/**
 * One provider's setup state from `GET /oauth/config`. Everything the
 * in-app "Set up sign-in" form renders comes from here — the frontend
 * holds zero provider knowledge. Secret values never appear on this surface.
 */
export interface OAuthProviderSummary {
  provider: 'twitch' | 'youtube' | 'kick' | 'facebook' | 'trovo';
  /** Real credentials present (user-entered, env, or release-embedded). */
  configured: boolean;
  /** Whether the credentials form must collect a client secret. */
  needsSecret: boolean;
  /** The stored client-id override, when the user entered one. */
  overrideClientId?: string | null;
  /** The provider's developer-portal page for registering an app. */
  registrationUrl: string;
  /** Pre-filled, copy-pasteable console fields + steps. */
  setup: OAuthProviderSetup;
}

/**
 * `startFlow` response — the BACKEND chooses the grant per provider:
 * - `flow: 'redirect'` → loopback authorization-code.
 * - `flow: 'device'`   → RFC 8628 device code (Twitch's mandated desktop
 *   sign-in); render `userCode` + `verificationUri` and wait for the
 *   `oauth_complete` event.
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
  isConfigured: async (provider: string): Promise<boolean> => {
    const { data } = await v1OauthIsConfiguredProxy({ path: { provider }, throwOnError: true });
    return data.configured;
  },
  startFlow: async (provider: string): Promise<OAuthFlowStarted> => {
    const { data } = await v1OauthStartFlowProxy({ path: { provider }, throwOnError: true });
    return data as OAuthFlowStarted;
  },
  completeFlow: async (
    provider: string,
    code: string,
    state: string
  ): Promise<{ provider: string; userId: string; username: string; displayName: string }> => {
    const { data } = await v1OauthCompleteFlowProxy({
      path: { provider },
      body: { code, state },
      throwOnError: true,
    });
    return data;
  },
  getAccount: async (provider: string): Promise<OAuthAccountStatus> => {
    const { data } = await v1OauthGetAccountProxy({ path: { provider }, throwOnError: true });
    return data as OAuthAccountStatus;
  },
  disconnect: async (provider: string): Promise<void> => {
    await v1OauthDisconnectProxy({ path: { provider }, throwOnError: true });
  },
  forget: async (provider: string): Promise<void> => {
    await v1OauthForgetProxy({ path: { provider }, throwOnError: true });
  },
  refreshToken: async (
    provider: string,
    refreshToken: string
  ): Promise<{ accessToken: string; refreshToken?: string; expiresIn?: number }> => {
    const { data } = await v1OauthRefreshTokenProxy({
      path: { provider },
      body: { refreshToken },
      throwOnError: true,
    });
    return {
      accessToken: data.accessToken,
      refreshToken: data.refreshToken ?? undefined,
      expiresIn: data.expiresIn ?? undefined,
    };
  },
  getConfig: async (): Promise<OAuthProviderSummary[]> => {
    const { data } = await v1OauthGetConfigProxy({ throwOnError: true });
    return data as unknown as OAuthProviderSummary[];
  },
  /**
   * Store one provider's client credentials (the in-app setup form).
   * Persisted server-side via the secret store. Empty/absent values clear
   * the stored override. Returns the updated summaries.
   */
  setProviderCredentials: async (
    provider: string,
    credentials: { clientId?: string; clientSecret?: string }
  ): Promise<OAuthProviderSummary[]> => {
    const { data } = await v1OauthSetProviderCredentialsProxy({
      path: { provider },
      body: credentials,
      throwOnError: true,
    });
    return data as unknown as OAuthProviderSummary[];
  },
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
  }): Promise<void> => {
    await v1OauthSetConfigProxy({ body: config, throwOnError: true });
  },
};
