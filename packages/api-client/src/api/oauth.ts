import type { OAuthAccountStatus } from '@spiritstream/types';
import { fetchTypedJson } from './_internal';

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
 * holds zero provider knowledge (which fields to show, what to paste,
 * where to register). Secret values never appear on this surface.
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
  getConfig: () => fetchTypedJson<OAuthProviderSummary[]>('GET', '/api/v1/oauth/config'),
  /**
   * Store one provider's client credentials (the in-app setup form).
   * Persisted server-side via the secret store — survives restarts.
   * Empty/absent values clear the stored override. Returns the updated
   * summaries so the caller can refresh without a second round-trip.
   */
  setProviderCredentials: (
    provider: string,
    credentials: { clientId?: string; clientSecret?: string }
  ) =>
    fetchTypedJson<OAuthProviderSummary[]>(
      'PUT',
      `/api/v1/oauth/config/${encodeURIComponent(provider)}`,
      undefined,
      credentials
    ),
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
