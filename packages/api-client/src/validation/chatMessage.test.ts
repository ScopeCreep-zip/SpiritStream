import { describe, expect, it, vi } from 'vitest';
import { parseChatMessage, parseChatMessages } from './chatMessage';

const valid = {
  id: 'm1',
  platform: 'twitch',
  username: 'viewer',
  message: 'hello',
  timestamp: 1_700_000_000_000,
  flags: 0,
  direction: 'inbound',
};

describe('parseChatMessage (single, live WS path)', () => {
  it('accepts a well-formed message', () => {
    expect(parseChatMessage(valid)).toEqual(valid);
  });

  it('drops a message missing a required scalar', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    expect(parseChatMessage({ ...valid, id: undefined })).toBeNull();
    expect(parseChatMessage({ ...valid, message: 42 })).toBeNull();
    expect(parseChatMessage(null)).toBeNull();
    warn.mockRestore();
  });
});

describe('parseChatMessages (array, REST replay/search)', () => {
  it('returns [] for non-array payloads', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    expect(parseChatMessages(null)).toEqual([]);
    expect(parseChatMessages({})).toEqual([]);
    warn.mockRestore();
  });

  it('keeps only well-formed messages and drops malformed ones', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const out = parseChatMessages([valid, { id: 'bad' }, { ...valid, id: 'm2' }]);
    expect(out.map((m) => m.id)).toEqual(['m1', 'm2']);
    warn.mockRestore();
  });

  it('preserves structured/nested fields it does not strictly validate', () => {
    const withExtras = { ...valid, author: { login: 'hash:abc' }, fragments: [{ kind: 'text' }] };
    const [out] = parseChatMessages([withExtras]);
    expect(out).toMatchObject({ author: { login: 'hash:abc' }, fragments: [{ kind: 'text' }] });
  });
});
