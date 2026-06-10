import { describe, it, expect } from 'vitest';
import { MessageFlag, hasFlag } from './messageFlags';

// N5: pure bitflag helper. The constants mirror the Rust `MessageFlags`
// enum byte-for-byte (see the module header) — a drifted shift here
// would silently mislabel every chat row that carries the flag. These
// tests pin the contract `hasFlag` actually implements: ALL bits in the
// mask must be set, not just any.

describe('hasFlag', () => {
  it('returns true when the single requested bit is set', () => {
    expect(hasFlag(MessageFlag.DISABLED, MessageFlag.DISABLED)).toBe(true);
  });

  it('returns false when the requested bit is clear', () => {
    expect(hasFlag(MessageFlag.HIGHLIGHTED, MessageFlag.DISABLED)).toBe(false);
  });

  it('isolates one flag from an unrelated flag sharing no bits', () => {
    const flags = MessageFlag.HIGHLIGHTED | MessageFlag.SUBSCRIPTION;
    expect(hasFlag(flags, MessageFlag.HIGHLIGHTED)).toBe(true);
    expect(hasFlag(flags, MessageFlag.SUBSCRIPTION)).toBe(true);
    expect(hasFlag(flags, MessageFlag.DISABLED)).toBe(false);
  });

  it('requires EVERY bit of a composite mask to be present', () => {
    const mask = MessageFlag.DISABLED | MessageFlag.TIMED_OUT_AUTHOR;
    // Only one of the two mask bits set → not a full match.
    expect(hasFlag(MessageFlag.DISABLED, mask)).toBe(false);
    // Both set → match.
    expect(hasFlag(MessageFlag.DISABLED | MessageFlag.TIMED_OUT_AUTHOR, mask)).toBe(true);
  });

  it('treats a zero flag set as having nothing set', () => {
    expect(hasFlag(0, MessageFlag.HIGHLIGHTED)).toBe(false);
  });
});

describe('MessageFlag bit layout', () => {
  it('assigns each flag a distinct power-of-two bit', () => {
    const values = Object.values(MessageFlag);
    const unique = new Set(values);
    expect(unique.size).toBe(values.length);
    for (const v of values) {
      // power of two: exactly one bit set
      expect(v & (v - 1)).toBe(0);
    }
  });

  it('pins the highest defined bit (REDEEMED_CHANNEL_POINT_REWARD = 1<<16)', () => {
    expect(MessageFlag.REDEEMED_CHANNEL_POINT_REWARD).toBe(1 << 16);
  });
});
