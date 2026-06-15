import { describe, expect, it } from 'vitest';
import { configureGeneratedClient, toApiError } from './clientConfig';
import { client } from './generated/client.gen';
import { v1SystemAppVersion } from './generated';
import { setBackendBaseUrl } from './config';

/**
 * Error-mapping contract. The generated client only types 2xx; this is the
 * single place non-2xx responses become the `ApiError` (`err.kind`) shape the
 * UI branches on. Ported from the removed `_internal.test.ts` when the
 * hand-rolled request layer was replaced by the generated client.
 */
describe('toApiError', () => {
  it('maps a structured { kind, message, details } body', async () => {
    const body = JSON.stringify({
      kind: 'invalid_stream_config',
      message: 'bad bitrate',
      details: { reasons: ['too high'] },
    });
    const err = await toApiError(new Response(body, { status: 422 }));
    expect(err.kind).toBe('invalid_stream_config');
    expect(err.status).toBe(422);
    expect(err.details).toEqual({ reasons: ['too high'] });
    expect(err.message).toBe('bad bitrate');
  });

  it('falls back to kind from body even without a message', async () => {
    const err = await toApiError(
      new Response(JSON.stringify({ kind: 'password_required' }), { status: 401 })
    );
    expect(err.kind).toBe('password_required');
    expect(err.status).toBe(401);
  });

  it.each([
    [401, 'unauthorized'],
    [403, 'forbidden'],
    [404, 'not_found'],
    [405, 'method_not_allowed'],
    [429, 'rate_limited'],
    [500, 'server_error'],
    [502, 'server_error'],
    [418, 'http_error'],
  ])('maps bodyless HTTP %i to kind %s', async (status, kind) => {
    const err = await toApiError(new Response(null, { status }));
    expect(err.kind).toBe(kind);
    expect(err.status).toBe(status);
  });

  it('uses status-derived kind when the body is non-JSON', async () => {
    const err = await toApiError(new Response('<html>gateway timeout</html>', { status: 504 }));
    expect(err.kind).toBe('server_error');
    expect(err.status).toBe(504);
  });
});

/**
 * Wiring test: `configureGeneratedClient` registers the response interceptor +
 * base config on the shared client. Exercised against a stubbed `fetch` (no
 * server) so we verify the interceptor actually throws the mapped `ApiError`
 * on non-2xx and passes 2xx through — the runtime seam typecheck can't prove.
 */
describe('configureGeneratedClient interceptor wiring', () => {
  it('rejects non-2xx with the mapped err.kind', async () => {
    configureGeneratedClient();
    setBackendBaseUrl('http://127.0.0.1:1');
    client.setConfig({
      fetch: async () => new Response(JSON.stringify({ kind: 'not_found' }), { status: 404 }),
    });
    await expect(v1SystemAppVersion({ throwOnError: true })).rejects.toMatchObject({
      kind: 'not_found',
      status: 404,
    });
  });

  it('passes a 2xx response through to data', async () => {
    configureGeneratedClient();
    client.setConfig({
      fetch: async () =>
        new Response(JSON.stringify({ version: '9.9.9' }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        }),
    });
    const { data } = await v1SystemAppVersion({ throwOnError: true });
    expect(data).toEqual({ version: '9.9.9' });
  });
});
