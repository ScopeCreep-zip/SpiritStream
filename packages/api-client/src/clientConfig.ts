/**
 * Central configuration for the generated @hey-api client — the ONE place
 * cross-cutting HTTP concerns live now that the typed surface is generated
 * from the server's OpenAPI spec (utoipa). This replaces the hand-rolled
 * `fetchTypedJson` request layer:
 *
 * - **baseUrl** tracks the runtime-discovered backend URL (`getBackendBaseUrl`)
 *   plus the `/api/v1` version prefix, re-applied whenever the URL changes
 *   (Tauri shell discovery, settings change, reset).
 * - **credentials: 'include'** carries the HttpOnly session cookie on every
 *   request — the only auth mechanism (`getAuthHeaders()` is intentionally
 *   empty; auth is cookie-based, not header-based).
 * - **fetch: safeFetch** keeps the localhost-startup-race retry + RFC 9110
 *   `Retry-After` handling (`safeFetch` is `Request`-aware and clones per
 *   retry attempt).
 * - **response interceptor** maps a non-2xx `{ kind, details }` body (or a
 *   bodyless HTTP error) into the `ApiError` shape (`Error & { kind, status,
 *   details }`) that UI code branches on. OpenAPI generators only type 2xx
 *   responses, so this preserves the structured-error contract the old
 *   `fetchTypedJson` provided.
 */

import { client } from './generated/client.gen';
import { onBackendBaseUrlChange, safeFetch } from './config';

/** Structured API error surfaced to callers — mirrors the shape the
 *  hand-rolled `fetchTypedJson` threw, so existing `err.kind` branching in
 *  the UI (e.g. `password_incorrect`, `unauthorized`) keeps working. */
export interface ApiError extends Error {
  kind: string;
  status: number;
  details?: unknown;
}

/** Map a bodyless HTTP error status to a stable `kind`, matching the tags
 *  the backend's `ApiError(CoreError)::IntoResponse` would emit. */
function kindForStatus(status: number): string {
  switch (status) {
    case 401:
      return 'unauthorized';
    case 403:
      return 'forbidden';
    case 404:
      return 'not_found';
    case 405:
      return 'method_not_allowed';
    case 429:
      return 'rate_limited';
    default:
      return status >= 500 ? 'server_error' : 'http_error';
  }
}

/** Build an `ApiError` from a non-2xx response. Prefers the structured
 *  `{ kind, message?, details? }` body the typed REST handlers emit; falls
 *  back to a status-derived `kind` for bodyless errors (405, proxy 502/504).
 *  Exported for unit testing — it carries the structured-error contract the
 *  whole UI branches on (previously covered by `_internal.test.ts`). */
export async function toApiError(response: Response): Promise<ApiError> {
  const text = await response.text().catch(() => '');
  let parsed: unknown;
  if (text) {
    try {
      parsed = JSON.parse(text);
    } catch {
      // Non-JSON error body — fall through to the status-derived kind.
    }
  }

  if (parsed && typeof parsed === 'object' && 'kind' in parsed) {
    const body = parsed as { kind: string; message?: string; details?: unknown };
    const err = new Error(body.message ?? body.kind) as ApiError;
    err.kind = body.kind;
    err.status = response.status;
    err.details = body.details;
    return err;
  }

  const err = new Error(`${response.status} ${response.statusText || 'HTTP error'}`) as ApiError;
  err.kind = kindForStatus(response.status);
  err.status = response.status;
  return err;
}

let configured = false;

/**
 * Configure the shared generated client. Idempotent — safe to call from the
 * package entry at import time; subsequent calls are no-ops.
 */
export function configureGeneratedClient(): void {
  if (configured) return;
  configured = true;

  client.setConfig({
    // Cookie-based session auth: the HttpOnly cookie rides every request.
    credentials: 'include',
    // Preserve the localhost startup-race retry + Retry-After semantics.
    fetch: safeFetch,
  });

  // Keep `baseUrl` pinned to the runtime-discovered backend + version prefix.
  // Fires immediately with the current value and again on every change.
  onBackendBaseUrlChange((base) => {
    client.setConfig({ baseUrl: `${base}/api/v1` });
  });

  // Non-2xx → throw the structured `ApiError` the UI branches on. Runs in the
  // response chain before the SDK parses `.data`, so the body is consumed
  // exactly once and the throw short-circuits success parsing.
  client.interceptors.response.use(async (response: Response) => {
    if (!response.ok) {
      throw await toApiError(response);
    }
    return response;
  });
}
