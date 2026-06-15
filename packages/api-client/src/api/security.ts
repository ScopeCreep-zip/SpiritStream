import { authLogout, confirmTokenIssue, securityRevokeAllSessions } from '../generated';
import { confirmTokenHeader } from './_confirm';

export const security = {
  /**
   * End the CURRENT session: the server removes it from the session
   * store and expires the HttpOnly cookie. Other devices' sessions
   * stay valid — use `revokeAllSessions` for those.
   */
  logout: async (): Promise<Record<string, never>> => {
    await authLogout({ throwOnError: true });
    return {};
  },
  /**
   * Request a one-shot confirmation token for the given destructive
   * intent. The token expires after `expiresInSeconds` and is
   * consumed by the destructive endpoint's `X-Confirm-Token` header.
   *
   * Most callers don't need this directly — use the wrapped destructive
   * methods (`settings.clearData`, `settings.rotateMachineKey`,
   * `security.revokeAllSessions`), which do the token dance internally.
   */
  requestConfirmToken: async (intent: string) => {
    const { data } = await confirmTokenIssue({ body: { intent }, throwOnError: true });
    return data;
  },
  /**
   * Revoke every active session server-side, logging out every device
   * that holds a session cookie / bearer token. The caller's own session
   * is invalidated; the UI should route to the login screen immediately
   * after. Wires the same confirm-token flow as `clearData` /
   * `rotateMachineKey`.
   */
  revokeAllSessions: async () => {
    const { data } = await securityRevokeAllSessions({
      headers: await confirmTokenHeader('revoke_all_sessions'),
      throwOnError: true,
    });
    return data;
  },
};
