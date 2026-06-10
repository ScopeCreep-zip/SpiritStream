import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

// N5: WebSocket event bus. Load-bearing behaviour: `events.on` opens a
// single shared socket, fans backend `{ event, payload }` frames to the
// matching handlers, and surfaces connection lifecycle as window
// CustomEvents (the Zustand connectionStore subscribes to these). A 1008
// close (auth failure) must raise `auth-required`, and dropping the last
// handler (without keepAlive) must close the socket so we don't hold an
// idle connection.

class FakeWebSocket {
  static OPEN = 1;
  static instances: FakeWebSocket[] = [];
  url: string;
  readyState = 0;
  private listeners: Record<string, Array<(e: unknown) => void>> = {};
  constructor(url: string) {
    this.url = url;
    FakeWebSocket.instances.push(this);
  }
  addEventListener(type: string, cb: (e: unknown) => void): void {
    (this.listeners[type] ||= []).push(cb);
  }
  close(): void {
    this.readyState = 3;
  }
  send(): void {}
  fire(type: string, event: unknown = {}): void {
    if (type === 'open') this.readyState = FakeWebSocket.OPEN;
    for (const cb of this.listeners[type] ?? []) cb(event);
  }
}

// The cross-origin path fetches a one-shot WS ticket before connecting;
// stub the REST call so tests stay network-free.
vi.mock('./api/_internal', () => ({
  fetchTypedJson: vi.fn().mockResolvedValue({ ticket: 'test-ticket', expiresInSeconds: 30 }),
}));

type EventsModule = typeof import('./events');
let mod: EventsModule;
let ac: AbortController;

function watch(): string[] {
  const seen: string[] = [];
  for (const n of ['connecting', 'connected', 'reconnected', 'disconnected', 'auth-required']) {
    window.addEventListener(`backend:${n}`, () => seen.push(n), { signal: ac.signal });
  }
  return seen;
}

const lastSocket = (): FakeWebSocket => FakeWebSocket.instances.at(-1)!;

/// The socket is created asynchronously (the cross-origin path fetches a
/// one-shot ticket first), so tests flush microtasks until it exists.
async function socketCreated(): Promise<FakeWebSocket> {
  for (let i = 0; i < 20 && FakeWebSocket.instances.length === 0; i++) {
    await Promise.resolve();
  }
  expect(FakeWebSocket.instances.length).toBeGreaterThan(0);
  return lastSocket();
}

beforeEach(async () => {
  vi.resetModules();
  FakeWebSocket.instances = [];
  (globalThis as { WebSocket: unknown }).WebSocket = FakeWebSocket;
  window.localStorage.clear();
  ac = new AbortController();
  mod = await import('./events');
});

afterEach(() => {
  ac.abort();
  vi.restoreAllMocks();
});

describe('events.on', () => {
  it('opens one socket, dispatches connecting→connected, and routes frames', async () => {
    const seen = watch();
    const handler = vi.fn();
    const onPromise = mod.events.on<{ a: number }>('chat_message', handler);
    (await socketCreated()).fire('open');
    await onPromise;

    expect(seen).toEqual(['connecting', 'connected']);
    expect(FakeWebSocket.instances).toHaveLength(1);

    lastSocket().fire('message', { data: JSON.stringify({ event: 'chat_message', payload: { a: 1 } }) });
    expect(handler).toHaveBeenCalledWith({ a: 1 });
  });

  it('ignores empty, unknown-event, and malformed frames', async () => {
    const handler = vi.fn();
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const onPromise = mod.events.on('known', handler);
    (await socketCreated()).fire('open');
    await onPromise;

    lastSocket().fire('message', { data: '' });
    lastSocket().fire('message', { data: JSON.stringify({ event: 'other', payload: 1 }) });
    lastSocket().fire('message', { data: 'not-json{' });

    expect(handler).not.toHaveBeenCalled();
    expect(warn).toHaveBeenCalled();
  });

  it('closes the socket when the last handler unsubscribes (no keepAlive)', async () => {
    const onPromise = mod.events.on('x', vi.fn());
    (await socketCreated()).fire('open');
    const unsub = await onPromise;
    const closeSpy = vi.spyOn(lastSocket(), 'close');
    unsub();
    expect(closeSpy).toHaveBeenCalled();
  });

  it('appends a one-shot ticket for a cross-origin socket', async () => {
    const onPromise = mod.events.on('x', vi.fn());
    const sock = await socketCreated();
    sock.fire('open');
    await onPromise;
    // jsdom's window.location.host differs from the backend ws host,
    // so the cross-origin branch runs and appends the fetched ticket.
    expect(sock.url).toContain('ticket=test-ticket');
  });

  it('raises auth-required on a 1008 close', async () => {
    const seen = watch();
    const onPromise = mod.events.on('x', vi.fn());
    (await socketCreated()).fire('open');
    await onPromise;
    lastSocket().fire('close', { code: 1008, reason: 'Unauthorized' });
    expect(seen).toContain('auth-required');
    expect(seen).toContain('disconnected');
  });
});

describe('initConnection / disconnectSocket', () => {
  it('initConnection opens a keepAlive socket', async () => {
    const seen = watch();
    mod.initConnection();
    (await socketCreated()).fire('open');
    // microtask flush so the open promise's notifyConnected runs
    await Promise.resolve();
    expect(seen).toContain('connected');
    expect(() => mod.disconnectSocket()).not.toThrow();
  });
});
