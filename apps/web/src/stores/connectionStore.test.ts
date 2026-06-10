import { describe, it, expect, beforeEach } from 'vitest';
import { useConnectionStore } from './connectionStore';

// N5: WebSocket connection-status store. The badge that tells a
// vulnerable streamer whether their events socket is live reads this
// store, so the state transitions are load-bearing UX. These tests pin
// the three setters and the `lastConnected` / `error` bookkeeping that
// the badge uses to distinguish "first connect" from "reconnecting".

beforeEach(() => {
  useConnectionStore.setState({ status: 'disconnected', lastConnected: null, error: null });
});

describe('connectionStore', () => {
  it('starts disconnected with no timestamp or error', () => {
    const s = useConnectionStore.getState();
    expect(s.status).toBe('disconnected');
    expect(s.lastConnected).toBeNull();
    expect(s.error).toBeNull();
  });

  it('setConnected records a timestamp and clears any prior error', () => {
    useConnectionStore.getState().setDisconnected('socket closed');
    useConnectionStore.getState().setConnected();
    const s = useConnectionStore.getState();
    expect(s.status).toBe('connected');
    expect(s.lastConnected).toBeInstanceOf(Date);
    expect(s.error).toBeNull();
  });

  it('setDisconnected stores the error string when given one', () => {
    useConnectionStore.getState().setDisconnected('handshake failed');
    const s = useConnectionStore.getState();
    expect(s.status).toBe('disconnected');
    expect(s.error).toBe('handshake failed');
  });

  it('setDisconnected with no argument leaves error null, not undefined', () => {
    useConnectionStore.getState().setDisconnected();
    const s = useConnectionStore.getState();
    expect(s.status).toBe('disconnected');
    expect(s.error).toBeNull();
  });

  it('setConnecting flips status without touching lastConnected', () => {
    useConnectionStore.getState().setConnected();
    const before = useConnectionStore.getState().lastConnected;
    useConnectionStore.getState().setConnecting();
    const s = useConnectionStore.getState();
    expect(s.status).toBe('connecting');
    // A reconnect attempt must preserve the prior successful-connect time.
    expect(s.lastConnected).toBe(before);
  });
});
