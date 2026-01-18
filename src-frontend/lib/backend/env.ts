export type BackendMode = 'tauri' | 'http';

export const backendMode: BackendMode = (() => {
  const mode = import.meta.env.VITE_BACKEND_MODE;
  if (mode === 'tauri' || mode === 'http') {
    return mode;
  }

  if (
    typeof window !== 'undefined' &&
    ('__TAURI__' in window || '__TAURI_INTERNALS__' in window || '__TAURI_IPC__' in window)
  ) {
    return 'tauri';
  }

  return 'http';
})();

export const backendUrlStorageKey = 'spiritstream-backend-url';
export const backendTokenStorageKey = 'spiritstream-backend-token';

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

const readTokenFromUrl = (): string | null => {
  if (typeof window === 'undefined') return null;
  try {
    const params = new URLSearchParams(window.location.search);
    const token = params.get('token');
    if (!token) return null;
    try {
      window.localStorage.setItem(backendTokenStorageKey, token);
    } catch {
      // Ignore storage errors.
    }
    return token;
  } catch {
    return null;
  }
};

const urlToken = readTokenFromUrl();

export const getBackendBaseUrl = (): string => {
  const stored = readStorageValue(backendUrlStorageKey);
  return stored || import.meta.env.VITE_BACKEND_URL || inferredBaseUrl;
};

export const getBackendToken = (): string => {
  if (urlToken) return urlToken;
  const stored = readStorageValue(backendTokenStorageKey);
  return stored || import.meta.env.VITE_BACKEND_TOKEN || '';
};

const defaultWsUrl = (baseUrl: string) => {
  const wsBase = baseUrl.replace(/^http/, 'ws').replace(/\/$/, '');
  return `${wsBase}/ws`;
};

export const getBackendWsUrl = (): string => {
  const baseUrl = getBackendBaseUrl();
  const wsUrl = import.meta.env.VITE_BACKEND_WS_URL || defaultWsUrl(baseUrl);
  const token = getBackendToken();
  if (!token) return wsUrl;

  try {
    const url = new URL(wsUrl);
    url.searchParams.set('token', token);
    return url.toString();
  } catch {
    const separator = wsUrl.includes('?') ? '&' : '?';
    return `${wsUrl}${separator}token=${encodeURIComponent(token)}`;
  }
};
