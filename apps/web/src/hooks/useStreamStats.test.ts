import { describe, it, expect } from 'vitest';
import { formatUptime, formatBitrate, formatSize } from './useStreamStats';

// N3: anchor test for the Vitest runner. `useStreamStats` exports a
// few pure formatters that are easy to pin without React rendering;
// they also catch real bugs (locale-sensitive formatting, off-by-one
// in the HH:MM:SS pad logic) so this isn't a smoke test.

describe('formatUptime', () => {
  it('renders MM:SS for sub-hour durations', () => {
    expect(formatUptime(0)).toBe('00:00');
    expect(formatUptime(5)).toBe('00:05');
    expect(formatUptime(59)).toBe('00:59');
    expect(formatUptime(60)).toBe('01:00');
    expect(formatUptime(3599)).toBe('59:59');
  });

  it('switches to HH:MM:SS once an hour elapses', () => {
    expect(formatUptime(3600)).toBe('01:00:00');
    expect(formatUptime(3661)).toBe('01:01:01');
    expect(formatUptime(36000)).toBe('10:00:00');
  });
});

describe('formatBitrate', () => {
  it('reports sub-Mbps values as kbps', () => {
    expect(formatBitrate(0)).toBe('0 kbps');
    expect(formatBitrate(500)).toBe('500 kbps');
    expect(formatBitrate(999)).toBe('999 kbps');
  });

  it('switches to Mbps at 1000 kbps', () => {
    expect(formatBitrate(1000)).toBe('1.0 Mbps');
    expect(formatBitrate(6500)).toBe('6.5 Mbps');
  });
});

describe('formatSize', () => {
  it('chooses the smallest unit that fits the value', () => {
    expect(formatSize(0)).toBe('0 B');
    expect(formatSize(1023)).toBe('1023 B');
    expect(formatSize(1024)).toBe('1.0 KB');
    expect(formatSize(1024 * 1024)).toBe('1.0 MB');
    expect(formatSize(1024 * 1024 * 1024)).toBe('1.00 GB');
  });
});
