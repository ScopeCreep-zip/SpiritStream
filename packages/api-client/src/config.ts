/** Base delay (ms) for HTTP localhost retries on network-level failure. */
const RETRY_BASE_DELAY_MS = 800;

/**
 * HTTP statuses where the server *may* be saying "retry later" — but per
 * RFC 9110 we only retry when the response includes a `Retry-After`
 * header. Absent that, the condition is permanent until something changes
 * externally; blind retry just amplifies load.
 */
const RETRY_AFTER_AWARE_STATUSES = new Set([502, 503, 504]);

/** Upper bound on `Retry-After` values we honor. Beyond this the server is
 *  telling us to wait too long; let the caller decide. */
const RETRY_AFTER_MAX_SECS = 30;

/**
 * Backend communication mode.
 *
 * HTTP is the only transport today. Tauri webview (desktop + mobile), Docker,
 * and browser deployments all hit the same Axum server. A future Veilid
 * transport will be selected via `makeApiClient({ transport: 'veilid' })`,
 * not via this mode flag.
 */
export type BackendMode = 'http';

/** True when the page is running inside a Tauri 2 webview (desktop or mobile). */
export const isTauri = (): boolean => {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
};

export const backendMode: BackendMode = 'http';

// ============================================================================
// Fetch Wrapper
// ============================================================================

/**
 * Fetch wrapper with retry logic for localhost requests.
 *
 * Handles race conditions where the frontend loads before the
 * backend server is fully ready (common in Tauri desktop app).
 *
 * For localhost requests, we always use the browser's native fetch().
 * The Tauri HTTP plugin has known bugs with localhost/127.0.0.1 requests:
 * - https://github.com/tauri-apps/plugins-workspace/issues/1484
 * - https://github.com/tauri-apps/plugins-workspace/issues/1559
 *
 * Since the CSP in tauri.conf.json allows any loopback port
 * (`http://127.0.0.1:*`), browser fetch works fine and is more reliable.
 *
 * For external URLs in Tauri context, the HTTP plugin is used to bypass
 * CORS/CSP restrictions (not currently used, but available for future needs).
 */
export async function safeFetch(url: string, options?: RequestInit): Promise<Response> {
  // For localhost requests, always use browser fetch with retry logic.
  // The Tauri HTTP plugin has known bugs with localhost/127.0.0.1.
  // Browser fetch works fine since CSP allows any loopback port.
  const isLocalhost = url.startsWith('http://127.0.0.1') || url.startsWith('http://localhost');

  if (isLocalhost) {
    // Retry logic for localhost - handles race condition with server startup
    // Increased retries to handle server warm-up time after health check passes
    const maxRetries = 5;
    const baseDelay = RETRY_BASE_DELAY_MS;

    // POST/PATCH are not idempotent: a 504 (upstream timeout) or a
    // connection dropped mid-response can mean the server already
    // executed the request — a blind retry would start a second stream
    // or send a chat message twice. Those fail straight to the caller.
    const method = (options?.method ?? 'GET').toUpperCase();
    const idempotent = method !== 'POST' && method !== 'PATCH';

    for (let attempt = 1; attempt <= maxRetries; attempt++) {
      try {
        const response = await fetch(url, options);

        // RFC 9110 — only retry 5xx when the server provided
        // `Retry-After`. A 5xx without `Retry-After` is a "fix it
        // yourself" condition (e.g. FfmpegNotFound) where retry just
        // amplifies load and floods the console with errors.
        if (idempotent && RETRY_AFTER_AWARE_STATUSES.has(response.status) && attempt < maxRetries) {
          const retryAfterHeader = response.headers.get('Retry-After');
          if (retryAfterHeader) {
            const retryAfterSec = Number.parseInt(retryAfterHeader, 10);
            if (
              Number.isFinite(retryAfterSec) &&
              retryAfterSec >= 0 &&
              retryAfterSec <= RETRY_AFTER_MAX_SECS
            ) {
              await new Promise((resolve) =>
                setTimeout(resolve, Math.max(retryAfterSec * 1000, 500))
              );
              continue;
            }
          }
          // No Retry-After or unreasonable value → return without retrying.
        }

        return response;
      } catch (error) {
        const isLastAttempt = attempt === maxRetries;

        if (isLastAttempt || !idempotent) {
          throw error;
        }

        // Network-level retry for localhost-startup races: exponential
        // backoff 800ms → 6.4s. This path is only reached when fetch()
        // throws (TCP refused / abort / DNS), not for HTTP responses.
        const delay = baseDelay * Math.pow(2, attempt - 1);
        await new Promise((resolve) => setTimeout(resolve, delay));
      }
    }

    // This shouldn't be reached, but TypeScript needs it
    throw new Error('safeFetch: unexpected state');
  }

  // External URLs always use browser fetch — Tauri 2 webview supports it
  // natively and CSP is configured to allow the localhost backend.
  return fetch(url, options);
}

export const backendUrlStorageKey = 'spiritstream-backend-url';

const defaultBaseUrl = 'http://127.0.0.1:8008';

// Infer the backend URL when nothing more authoritative is configured:
// - PROD bundle outside Tauri: the only shape that exists is the backend
//   serving its own UI (`SPIRITSTREAM_UI_ENABLED`), so the page origin IS
//   the API — works on any port, including an OS-assigned one.
// - DEV (vite) outside Tauri: a separately started backend, conventional
//   fixed port (`SPIRITSTREAM_PORT=8008`, see .env.example).
// - Inside Tauri the shell-discovered URL arrives via `setBackendBaseUrl`
//   before anything renders, so this inference is never consulted.
const inferredBaseUrl = (): string => {
  if (typeof window === 'undefined') return defaultBaseUrl;
  if (import.meta.env.PROD && !isTauri()) return window.location.origin;
  return defaultBaseUrl;
};

const readStorageValue = (key: string): string | null => {
  if (typeof window === 'undefined') return null;
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
};

// Runtime-discovered backend URL — set once at bootstrap from the Tauri
// shell's `backend_url` command (the port is OS-negotiated per launch).
let runtimeBaseUrl: string | null = null;

/**
 * Configure the backend URL discovered at runtime (Tauri desktop: the
 * shell reads the OS-negotiated port from `run/server.port` and hands
 * it over via the `backend_url` command before the app renders).
 *
 * Takes precedence over EVERYTHING, including localStorage — a port
 * stored during a previous launch is stale by construction when ports
 * are negotiated per launch.
 */
export function setBackendBaseUrl(url: string): void {
  runtimeBaseUrl = url.replace(/\/$/, '');
}

export const getBackendBaseUrl = (): string => {
  if (runtimeBaseUrl) return runtimeBaseUrl;
  const stored = readStorageValue(backendUrlStorageKey);
  return stored || import.meta.env.VITE_BACKEND_URL || inferredBaseUrl();
};

const defaultWsUrl = (baseUrl: string) => {
  const wsBase = baseUrl.replace(/^http/, 'ws').replace(/\/$/, '');
  return `${wsBase}/api/v1/events`;
};

export const getBackendWsUrl = (): string => {
  const baseUrl = getBackendBaseUrl();
  return import.meta.env.VITE_BACKEND_WS_URL || defaultWsUrl(baseUrl);
};

/**
 * Update the backend URL when settings change.
 *
 * Call this when the user changes backendHost/backendPort in settings
 * to persist the new URL to localStorage. This ensures subsequent API
 * calls and WebSocket connections use the new address.
 *
 * Note: After calling this, the WebSocket should be reconnected to
 * use the new URL. Call `disconnectSocket()` followed by `initConnection()`
 * from httpEvents.ts to force reconnection.
 *
 * @param host - The new backend host (e.g., "127.0.0.1" or "192.168.1.100")
 * @param port - The new backend port (e.g., 8008)
 */
export function updateBackendUrl(host: string, port: number): void {
  if (typeof window === 'undefined') return;

  const newUrl = `http://${host}:${port}`;
  window.localStorage.setItem(backendUrlStorageKey, newUrl);
}

/**
 * Clear the stored backend URL, reverting to defaults.
 * Useful when resetting settings.
 */
export function clearBackendUrl(): void {
  if (typeof window === 'undefined') return;
  window.localStorage.removeItem(backendUrlStorageKey);
}

// ============================================================================
// Authentication API (HttpOnly Cookie-based)
// ============================================================================

export interface AuthStatus {
  authenticated: boolean;
  required: boolean;
}

/**
 * Check current authentication status with the backend.
 * Returns whether user is authenticated and whether auth is required.
 */
export async function checkAuth(): Promise<AuthStatus> {
  const baseUrl = getBackendBaseUrl();
  try {
    const response = await safeFetch(`${baseUrl}/api/v1/auth/check`, {
      method: 'GET',
      credentials: 'include', // Send cookies
    });

    if (!response.ok) {
      return { authenticated: false, required: true };
    }

    const data = await response.json();
    return {
      authenticated: data.authenticated ?? false,
      required: data.required ?? true,
    };
  } catch {
    return { authenticated: false, required: true };
  }
}

/**
 * Authenticate with the backend using an API token.
 * On success, the server sets an HttpOnly session cookie.
 *
 * @param token - The API token to authenticate with
 * @returns true if login succeeded, false otherwise
 */
export async function login(token: string): Promise<boolean> {
  const baseUrl = getBackendBaseUrl();
  try {
    const response = await safeFetch(`${baseUrl}/api/v1/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      credentials: 'include', // Receive and store cookies
      body: JSON.stringify({ token }),
    });

    return response.ok;
  } catch {
    return false;
  }
}

// ============================================================================
// Request Helpers
// ============================================================================

/**
 * Get authentication headers for API requests.
 * Since we use HttpOnly cookies for auth, this returns an empty object.
 * The actual auth is handled via credentials: 'include' on fetch requests.
 */
export function getAuthHeaders(): Record<string, string> {
  return {};
}

// ============================================================================
// Server Readiness — types only
// ============================================================================
//
// The frontend talks to `GET /api/v1/ready` directly via `fetch`. The server
// holds the connection (long-poll) until services initialize, then returns
// 200 ready=true. On timeout it returns 503 + `Retry-After`. Single-pattern
// readiness across every deployment; no helper functions needed here.

export interface ServerReadyError {
  check: string;
  error: string;
}

export interface ServerReadyStatus {
  ready: boolean;
  status?: number;
  failed?: string[];
  errors?: ServerReadyError[];
  lastError?: string;
}
