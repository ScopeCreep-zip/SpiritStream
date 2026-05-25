/**
 * Internal HTTP primitives for every api-namespace file in this directory.
 * NOT exported from the package — namespace files import from here, and
 * `../api.ts` re-exports the assembled `api` object.
 */

import { getAuthHeaders, getBackendBaseUrl, safeFetch } from '../config';

/**
 * Issue a typed-REST request and return the response body.
 *
 * Success: raw `T` from the handler's `Json<T>` response.
 * Error: `{ kind, details? }` per `ApiError(CoreError)::IntoResponse` in
 * `crates/transport-http/src/error.rs`. The thrown `Error` carries
 * `(err as Error & { kind, status, details? })` so callers can branch on
 * a specific validation failure without parsing the message string.
 */
export async function fetchTypedJson<T>(
  method: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE',
  path: string,
  query?: Record<string, string>,
  body?: unknown,
  /**
   * Extra request headers. Used for one-shot confirmation tokens
   * attached as `X-Confirm-Token` on destructive endpoints.
   * `getAuthHeaders()` still drives the session cookie / bearer; this
   * merges on top.
   */
  extraHeaders?: Record<string, string>,
): Promise<T> {
  const baseUrl = getBackendBaseUrl();
  const qs = query ? `?${new URLSearchParams(query).toString()}` : '';
  const headers: Record<string, string> = {
    ...getAuthHeaders(),
    ...(extraHeaders ?? {}),
  };
  if (body !== undefined) headers['Content-Type'] = 'application/json';

  const response = await safeFetch(`${baseUrl}${path}${qs}`, {
    method,
    headers,
    credentials: 'include',
    body: body === undefined ? undefined : JSON.stringify(body),
  });

  const text = await response.text();
  let parsed: unknown;
  if (text) {
    try {
      parsed = JSON.parse(text);
    } catch {
      throw new Error('Invalid response from server');
    }
  }

  if (!response.ok) {
    // Typed REST error: { kind, details? }
    if (parsed && typeof parsed === 'object' && 'kind' in parsed) {
      const body = parsed as { kind: string; message?: string; details?: unknown };
      const err = new Error(body.message ?? body.kind) as Error & {
        kind: string;
        status: number;
        details?: unknown;
      };
      err.kind = body.kind;
      err.status = response.status;
      err.details = body.details;
      throw err;
    }
    // Bodyless HTTP error (405 Method Not Allowed, 502/504 from proxies,
    // etc). Tag the thrown Error with `kind` + `status` so callers can
    // branch instead of string-matching the `statusText` fallback.
    const kind =
      response.status === 405
        ? 'method_not_allowed'
        : response.status === 401
          ? 'unauthorized'
          : response.status === 403
            ? 'forbidden'
            : response.status === 404
              ? 'not_found'
              : response.status === 429
                ? 'rate_limited'
                : response.status >= 500
                  ? 'server_error'
                  : 'http_error';
    const err = new Error(
      `${response.status} ${response.statusText || 'HTTP error'} for ${method} ${path}`,
    ) as Error & { kind: string; status: number };
    err.kind = kind;
    err.status = response.status;
    throw err;
  }

  // Typed REST success: handler's `Json<T>` value. Caller handles
  // `undefined` when `T = void` (bodyless 200/204 responses).
  return parsed as T;
}

/**
 * Wrap a destructive backend call with the confirm-token dance:
 *   1. Request a one-shot token for the given `intent` from
 *      `POST /api/v1/security/confirm-token`. Tokens are scoped to a
 *      single intent string (`clear_data`, `rotate_machine_key`,
 *      `revoke_all_sessions`) and expire after a short TTL.
 *   2. Invoke `call(headers)` with the token attached as
 *      `X-Confirm-Token`. The backend's `require_confirm_token`
 *      middleware consumes the token before the handler runs.
 *
 * Two-step pattern keeps the api-client side ergonomic: callers
 * just `withConfirmToken('intent', headers => fetchTypedJson(..., headers))`
 * without juggling the token issuance themselves.
 */
export async function withConfirmToken<T>(
  intent: string,
  call: (headers: Record<string, string>) => Promise<T>,
): Promise<T> {
  const { token } = await fetchTypedJson<{ token: string; expiresInSeconds: number }>(
    'POST',
    '/api/v1/security/confirm-token',
    undefined,
    { intent },
  );
  return call({ 'X-Confirm-Token': token });
}
