import { describe, it, expect, beforeEach, vi } from 'vitest';

// N5: typed-REST primitives. The load-bearing logic is the error
// mapping — a typed `{ kind, details }` body must surface as a tagged
// Error callers can branch on, and a bodyless HTTP error must still get
// a `kind`/`status` from the status code (so a 405/401/429 from a proxy
// isn't an opaque string). `withConfirmToken` must issue a one-shot
// token and attach it as `X-Confirm-Token` before the destructive call.

const { safeFetch } = vi.hoisted(() => ({ safeFetch: vi.fn() }));
vi.mock('../config', () => ({
  safeFetch,
  getBackendBaseUrl: () => 'http://127.0.0.1:8008',
  getAuthHeaders: () => ({}),
}));

import { fetchTypedJson, withConfirmToken } from './_internal';

interface FakeResponseInit {
  ok?: boolean;
  status?: number;
  statusText?: string;
  body?: string;
}

function fakeResponse(init: FakeResponseInit): Response {
  const { ok = true, status = 200, statusText = 'OK', body = '' } = init;
  return {
    ok,
    status,
    statusText,
    headers: { get: () => null },
    text: async () => body,
  } as unknown as Response;
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe('fetchTypedJson', () => {
  it('returns the parsed JSON body on success', async () => {
    safeFetch.mockResolvedValue(fakeResponse({ body: JSON.stringify({ id: 'p1' }) }));
    const out = await fetchTypedJson<{ id: string }>('GET', '/api/v1/profiles/p1');
    expect(out).toEqual({ id: 'p1' });
  });

  it('returns undefined for a bodyless 200 (T = void)', async () => {
    safeFetch.mockResolvedValue(fakeResponse({ body: '' }));
    const out = await fetchTypedJson<void>('DELETE', '/api/v1/profiles/p1');
    expect(out).toBeUndefined();
  });

  it('appends the query string and sets Content-Type when a body is sent', async () => {
    safeFetch.mockResolvedValue(fakeResponse({ body: '{}' }));
    await fetchTypedJson('POST', '/api/v1/chat/send', { platform: 'twitch' }, { message: 'hi' });
    const [url, opts] = safeFetch.mock.calls[0];
    expect(url).toBe('http://127.0.0.1:8008/api/v1/chat/send?platform=twitch');
    expect((opts as RequestInit).method).toBe('POST');
    expect((opts as RequestInit).body).toBe(JSON.stringify({ message: 'hi' }));
    expect((opts as { headers: Record<string, string> }).headers['Content-Type']).toBe(
      'application/json'
    );
  });

  it('throws "Invalid response from server" when the body is not JSON', async () => {
    safeFetch.mockResolvedValue(fakeResponse({ body: '<html>oops</html>' }));
    await expect(fetchTypedJson('GET', '/x')).rejects.toThrow('Invalid response from server');
  });

  it('surfaces a typed { kind, details } error with status attached', async () => {
    safeFetch.mockResolvedValue(
      fakeResponse({
        ok: false,
        status: 422,
        body: JSON.stringify({ kind: 'validation', message: 'bad name', details: { field: 'name' } }),
      })
    );
    await expect(fetchTypedJson('POST', '/api/v1/profiles')).rejects.toMatchObject({
      message: 'bad name',
      kind: 'validation',
      status: 422,
      details: { field: 'name' },
    });
  });

  it.each([
    [405, 'method_not_allowed'],
    [401, 'unauthorized'],
    [403, 'forbidden'],
    [404, 'not_found'],
    [429, 'rate_limited'],
    [503, 'server_error'],
    [418, 'http_error'],
  ])('maps a bodyless %i error to kind "%s"', async (status, kind) => {
    safeFetch.mockResolvedValue(fakeResponse({ ok: false, status, statusText: 'Err', body: '' }));
    await expect(fetchTypedJson('GET', '/x')).rejects.toMatchObject({ kind, status });
  });
});

describe('withConfirmToken', () => {
  it('issues a one-shot token then attaches it as X-Confirm-Token', async () => {
    safeFetch.mockResolvedValue(
      fakeResponse({ body: JSON.stringify({ token: 'tok-123', expiresInSeconds: 30 }) })
    );
    const call = vi.fn().mockResolvedValue('done');
    const result = await withConfirmToken('rotate_machine_key', call);
    expect(result).toBe('done');
    expect(call).toHaveBeenCalledWith({ 'X-Confirm-Token': 'tok-123' });
    // The token issuance hit the confirm-token endpoint with the intent.
    const [url, opts] = safeFetch.mock.calls[0];
    expect(url).toBe('http://127.0.0.1:8008/api/v1/security/confirm-token');
    expect((opts as RequestInit).body).toBe(JSON.stringify({ intent: 'rotate_machine_key' }));
  });
});
