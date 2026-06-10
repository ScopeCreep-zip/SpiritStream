import { fetchTypedJson, withConfirmToken } from './_internal';

export const security = {
  /**
   * Request a one-shot confirmation token for the given destructive
   * intent. The token expires after `expiresInSeconds` and is
   * consumed by the destructive endpoint's `X-Confirm-Token` header.
   *
   * Most callers don't need this directly — use the wrapped
   * destructive methods (`settings.clearData`,
   * `settings.rotateMachineKey`, `security.revokeAllSessions`)
   * which do the token dance internally via `withConfirmToken`.
   * Exposed for the rare case a custom intent needs custom handling.
   */
  requestConfirmToken: (intent: string) =>
    fetchTypedJson<{ token: string; expiresInSeconds: number }>(
      'POST',
      '/api/v1/security/confirm-token',
      undefined,
      { intent }
    ),
  /**
   * Revoke every active session server-side, effectively logging out
   * every device that holds a session cookie / bearer token. The
   * caller's own session is invalidated by this call; the UI should
   * route the user to the login screen immediately after.
   *
   * Wires the same confirm-token flow as `clearData` /
   * `rotateMachineKey`.
   */
  revokeAllSessions: () =>
    withConfirmToken<{ revoked: number }>('revoke_all_sessions', (headers) =>
      fetchTypedJson('POST', '/api/v1/security/sessions/revoke-all', undefined, undefined, headers)
    ),
};
