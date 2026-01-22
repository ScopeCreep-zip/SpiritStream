/**
 * Backend communication mode.
 *
 * - `'http'`: Default mode. All API calls go through HTTP/WebSocket to a
 *   backend server. Used by:
 *   - Desktop (Tauri spawns server sidecar)
 *   - Docker containers
 *   - Remote browser access
 *
 * - `'tauri'`: Legacy mode for testing. Uses Tauri IPC directly, bypassing
 *   the HTTP server. Only activated via `VITE_BACKEND_MODE=tauri` env var.
 *   Not recommended for production use.
 */
export type BackendMode = 'tauri' | 'http';

/**
 * Check if we're running inside a Tauri webview.
 * Note: This doesn't determine the backend mode - even in Tauri, we use HTTP
 * by default because the desktop app spawns a backend server.
 */
export const isTauri = (): boolean => {
  return typeof window !== 'undefined' && '__TAURI__' in window;
};

/**
 * The active backend communication mode.
 *
 * Defaults to 'http' because the desktop app architecture uses a Rust HTTP
 * server for all business logic. The frontend communicates via HTTP/WebSocket
 * regardless of whether it's running in Tauri, Docker, or a browser.
 *
 * Override with `VITE_BACKEND_MODE=tauri` for legacy testing only.
 */
export const backendMode: BackendMode = (() => {
  const mode = import.meta.env.VITE_BACKEND_MODE;
  if (mode === 'tauri' || mode === 'http') {
    return mode;
  }

  // Default to HTTP mode - the backend server handles all business logic
  return 'http';
})();

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
 * Since the CSP in tauri.conf.json already allows http://127.0.0.1:8008,
 * browser fetch works fine and is more reliable.
 *
 * For external URLs in Tauri context, the HTTP plugin is used to bypass
 * CORS/CSP restrictions (not currently used, but available for future needs).
 */
export async function safeFetch(
  url: string,
  options?: RequestInit
): Promise<Response> {
  // For localhost requests, always use browser fetch with retry logic.
  // The Tauri HTTP plugin has known bugs with localhost/127.0.0.1.
  // Browser fetch works fine since CSP allows localhost:8008.
  const isLocalhost =
    url.startsWith('http://127.0.0.1') || url.startsWith('http://localhost');

  if (isLocalhost) {
    // Retry logic for localhost - handles race condition with server startup
    // Increased retries to handle server warm-up time after health check passes
    const maxRetries = 5;
    const baseDelay = 800; // ms

    for (let attempt = 1; attempt <= maxRetries; attempt++) {
      try {
        const response = await fetch(url, options);
        return response;
      } catch (error) {
        const isLastAttempt = attempt === maxRetries;

        if (isLastAttempt) {
          console.error(
            `[safeFetch] All ${maxRetries} attempts failed for ${url}:`,
            error
          );
          throw error;
        }

        // Exponential backoff: 800ms, 1600ms, 3200ms, 6400ms
        const delay = baseDelay * Math.pow(2, attempt - 1);
        console.warn(
          `[safeFetch] Attempt ${attempt}/${maxRetries} failed, retrying in ${delay}ms...`
        );
        await new Promise((resolve) => setTimeout(resolve, delay));
      }
    }

    // This shouldn't be reached, but TypeScript needs it
    throw new Error('safeFetch: unexpected state');
  }

  // For external URLs in Tauri context, use the HTTP plugin
  // (not currently used, but available for future needs)
  if (isTauri()) {
    try {
      const { fetch: tauriFetch } = await import('@tauri-apps/plugin-http');
      return tauriFetch(url, options);
    } catch (error) {
      console.warn(
        '[safeFetch] Tauri HTTP plugin not available, falling back to browser fetch:',
        error
      );
      return fetch(url, options);
    }
  }

  return fetch(url, options);
}

export const backendUrlStorageKey = 'spiritstream-backend-url';

const defaultBaseUrl = 'http://127.0.0.1:8008';

// Infer the backend URL based on current context:
// - If we're running on port 8008, we're likely being served by the backend itself
// - Otherwise (e.g., Vite dev server on 1420), use the default backend URL
const inferredBaseUrl = (() => {
  if (typeof window === 'undefined') return defaultBaseUrl;
  const origin = window.location.origin;
  // If served directly by the backend (port 8008), use the origin
  if (origin.includes(':8008')) return origin;
  // Otherwise, use the default backend URL (dev server scenario)
  return defaultBaseUrl;
})();

const readStorageValue = (key: string): string | null => {
  if (typeof window === 'undefined') return null;
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
};

export const getBackendBaseUrl = (): string => {
  const stored = readStorageValue(backendUrlStorageKey);
  return stored || import.meta.env.VITE_BACKEND_URL || inferredBaseUrl;
};

const defaultWsUrl = (baseUrl: string) => {
  const wsBase = baseUrl.replace(/^http/, 'ws').replace(/\/$/, '');
  return `${wsBase}/ws`;
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

  console.log(`[env] Backend URL updated to: ${newUrl}`);
}

/**
 * Clear the stored backend URL, reverting to defaults.
 * Useful when resetting settings.
 */
export function clearBackendUrl(): void {
  if (typeof window === 'undefined') return;
  window.localStorage.removeItem(backendUrlStorageKey);
  console.log('[env] Backend URL cleared, will use defaults');
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
    const response = await safeFetch(`${baseUrl}/auth/check`, {
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
  } catch (error) {
    console.error('[auth] Failed to check auth status:', error);
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
    const response = await safeFetch(`${baseUrl}/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      credentials: 'include', // Receive and store cookies
      body: JSON.stringify({ token }),
    });

    if (!response.ok) {
      return false;
    }

    const data = await response.json();
    return data.ok === true;
  } catch (error) {
    console.error('[auth] Login failed:', error);
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
// Health Check API
// ============================================================================

/**
 * Fetch with timeout protection for health checks.
 * Unlike safeFetch(), this uses a single attempt with a hard timeout
 * to avoid long waits during initial connection checks.
 */
async function fetchWithTimeout(
  url: string,
  timeoutMs: number = 3000
): Promise<Response> {
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), timeoutMs);

  try {
    const response = await fetch(url, {
      method: 'GET',
      signal: controller.signal,
    });
    clearTimeout(timeoutId);
    return response;
  } catch (error) {
    clearTimeout(timeoutId);
    throw error;
  }
}

/**
 * Check if the backend server is reachable.
 * Uses a timeout-protected fetch to avoid hangs.
 */
export async function checkServerHealth(retries = 10, delayMs = 500): Promise<boolean> {
  const baseUrl = getBackendBaseUrl();

  for (let attempt = 1; attempt <= retries; attempt++) {
    try {
      const response = await fetchWithTimeout(`${baseUrl}/health`, 3000);

      if (response.ok) {
        return true;
      }
    } catch {
      // Network error or timeout - will retry
    }

    if (attempt < retries) {
      await new Promise((resolve) => setTimeout(resolve, delayMs));
    }
  }

  return false;
}

/**
 * Check if the backend server is fully ready to serve requests.
 * Uses a timeout-protected fetch to avoid hangs.
 */
export async function checkServerReady(retries = 15, delayMs = 300): Promise<boolean> {
  const baseUrl = getBackendBaseUrl();

  for (let attempt = 1; attempt <= retries; attempt++) {
    try {
      const response = await fetchWithTimeout(`${baseUrl}/ready`, 3000);

      if (response.ok) {
        // Parse JSON with a try-catch to handle malformed responses
        try {
          const text = await response.text();
          const data = JSON.parse(text);
          if (data.ready === true) {
            return true;
          }
        } catch {
          // JSON parse error - server might not be fully ready
        }
      }
    } catch {
      // Network error or timeout - will retry
    }

    if (attempt < retries) {
      await new Promise((resolve) => setTimeout(resolve, delayMs));
    }
  }

  return false;
}

