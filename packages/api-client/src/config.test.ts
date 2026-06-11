import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import {
  isTauri,
  getAuthHeaders,
  getBackendBaseUrl,
  getBackendWsUrl,
  updateBackendUrl,
  clearBackendUrl,
  backendUrlStorageKey,
  safeFetch,
  checkAuth,
  login,
} from './config';

// N5: api-client transport config. Pinned behaviour: backend-URL
// resolution precedence (stored override → inferred default), the
// http→ws derivation, and `safeFetch`'s localhost retry policy —
// network-throw uses exponential backoff, but a 5xx is only retried
// when the server sent a sane `Retry-After` (RFC 9110); otherwise it
// returns immediately so a permanent 5xx can't spin the retry loop.

const realFetch = globalThis.fetch;

function fakeResponse(init: {
  ok?: boolean;
  status?: number;
  retryAfter?: string | null;
  json?: unknown;
}): Response {
  const { ok = true, status = 200, retryAfter = null, json } = init;
  return {
    ok,
    status,
    headers: { get: (name: string) => (name === 'Retry-After' ? retryAfter : null) },
    json: async () => json,
  } as unknown as Response;
}

beforeEach(() => {
  window.localStorage.clear();
  vi.restoreAllMocks();
});

afterEach(() => {
  globalThis.fetch = realFetch;
  vi.useRealTimers();
});

describe('isTauri / getAuthHeaders', () => {
  it('isTauri is false outside a Tauri webview', () => {
    expect(isTauri()).toBe(false);
  });

  it('getAuthHeaders is empty (cookie auth)', () => {
    expect(getAuthHeaders()).toEqual({});
  });
});

describe('backend URL resolution', () => {
  it('falls back to the default localhost URL with no override', () => {
    expect(getBackendBaseUrl()).toBe('http://127.0.0.1:8008');
  });

  it('prefers a stored override and derives the ws URL from it', () => {
    updateBackendUrl('192.168.1.50', 9000);
    expect(window.localStorage.getItem(backendUrlStorageKey)).toBe('http://192.168.1.50:9000');
    expect(getBackendBaseUrl()).toBe('http://192.168.1.50:9000');
    expect(getBackendWsUrl()).toBe('ws://192.168.1.50:9000/api/v1/events');
  });

  it('clearBackendUrl reverts to the default', () => {
    updateBackendUrl('10.0.0.1', 8080);
    clearBackendUrl();
    expect(getBackendBaseUrl()).toBe('http://127.0.0.1:8008');
  });

  // Fresh module instance: setBackendBaseUrl writes module-level state
  // that must not leak into the singleton other tests share.
  it('runtime-discovered URL beats the localStorage override', async () => {
    vi.resetModules();
    const fresh = await import('./config');
    fresh.updateBackendUrl('10.0.0.9', 9999);
    fresh.setBackendBaseUrl('http://127.0.0.1:54321/');
    // Trailing slash is normalized; a port stored by a previous launch
    // is stale by construction when ports are negotiated per launch.
    expect(fresh.getBackendBaseUrl()).toBe('http://127.0.0.1:54321');
    expect(fresh.getBackendWsUrl()).toBe('ws://127.0.0.1:54321/api/v1/events');
  });
});

describe('safeFetch', () => {
  it('passes external URLs straight through to fetch', async () => {
    const resp = fakeResponse({});
    globalThis.fetch = vi.fn().mockResolvedValue(resp);
    const out = await safeFetch('https://example.com/x');
    expect(out).toBe(resp);
    expect(globalThis.fetch).toHaveBeenCalledTimes(1);
  });

  it('returns the first localhost response on success', async () => {
    const resp = fakeResponse({ status: 200 });
    globalThis.fetch = vi.fn().mockResolvedValue(resp);
    const out = await safeFetch('http://127.0.0.1:8008/api/v1/ready');
    expect(out).toBe(resp);
    expect(globalThis.fetch).toHaveBeenCalledTimes(1);
  });

  it('returns a 5xx immediately when there is no Retry-After header', async () => {
    const resp = fakeResponse({ ok: false, status: 503, retryAfter: null });
    globalThis.fetch = vi.fn().mockResolvedValue(resp);
    const out = await safeFetch('http://localhost:8008/x');
    expect(out).toBe(resp);
    expect(globalThis.fetch).toHaveBeenCalledTimes(1);
  });

  it('honors a sane Retry-After then returns the recovered response', async () => {
    vi.useFakeTimers();
    const busy = fakeResponse({ ok: false, status: 503, retryAfter: '1' });
    const ok = fakeResponse({ ok: true, status: 200 });
    globalThis.fetch = vi.fn().mockResolvedValueOnce(busy).mockResolvedValueOnce(ok);
    const p = safeFetch('http://127.0.0.1:8008/x');
    await vi.advanceTimersByTimeAsync(1000);
    await expect(p).resolves.toBe(ok);
    expect(globalThis.fetch).toHaveBeenCalledTimes(2);
  });

  it('retries network failures with backoff then succeeds', async () => {
    vi.useFakeTimers();
    const ok = fakeResponse({ ok: true, status: 200 });
    globalThis.fetch = vi
      .fn()
      .mockRejectedValueOnce(new Error('ECONNREFUSED'))
      .mockResolvedValueOnce(ok);
    const p = safeFetch('http://127.0.0.1:8008/x');
    await vi.advanceTimersByTimeAsync(800);
    await expect(p).resolves.toBe(ok);
    expect(globalThis.fetch).toHaveBeenCalledTimes(2);
  });

  it('throws after exhausting localhost retries', async () => {
    vi.useFakeTimers();
    globalThis.fetch = vi.fn().mockRejectedValue(new Error('ECONNREFUSED'));
    const p = safeFetch('http://127.0.0.1:8008/x');
    const assertion = expect(p).rejects.toThrow('ECONNREFUSED');
    // Backoff is 800 + 1600 + 3200 + 6400 across the 5 attempts.
    await vi.advanceTimersByTimeAsync(12000);
    await assertion;
    expect(globalThis.fetch).toHaveBeenCalledTimes(5);
  });
});

describe('checkAuth / login', () => {
  it('checkAuth returns the parsed status on a 200', async () => {
    globalThis.fetch = vi
      .fn()
      .mockResolvedValue(fakeResponse({ ok: true, json: { authenticated: true, required: false } }));
    await expect(checkAuth()).resolves.toEqual({ authenticated: true, required: false });
  });

  it('checkAuth treats a non-200 as unauthenticated+required', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(fakeResponse({ ok: false, status: 401 }));
    await expect(checkAuth()).resolves.toEqual({ authenticated: false, required: true });
  });

  it('login is true on a 200 and false on a non-200', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(fakeResponse({ ok: true }));
    await expect(login('tok')).resolves.toBe(true);
    globalThis.fetch = vi.fn().mockResolvedValue(fakeResponse({ ok: false, status: 403 }));
    await expect(login('tok')).resolves.toBe(false);
  });
});
