export type BackendMode = 'tauri' | 'http';

// The desktop app is a minimal launcher that spawns a backend server.
// All communication happens over HTTP, not Tauri IPC commands.
// The 'tauri' mode is only used if explicitly set via env var (for legacy/testing).
export const backendMode: BackendMode = (() => {
  const mode = import.meta.env.VITE_BACKEND_MODE;
  if (mode === 'tauri' || mode === 'http') {
    return mode;
  }

  // Default to HTTP mode - the backend server handles all business logic
  return 'http';
})();

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
    const response = await fetch(`${baseUrl}/auth/check`, {
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
    const response = await fetch(`${baseUrl}/auth/login`, {
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
 * Check if the backend server is reachable.
 * Used on startup to verify connection before rendering the app.
 */
export async function checkServerHealth(): Promise<boolean> {
  const baseUrl = getBackendBaseUrl();
  try {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 5000);

    const response = await fetch(`${baseUrl}/health`, {
      method: 'GET',
      signal: controller.signal,
    });

    clearTimeout(timeoutId);
    return response.ok;
  } catch (error) {
    console.error('[health] Server health check failed:', error);
    return false;
  }
}

