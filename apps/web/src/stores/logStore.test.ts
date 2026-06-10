import { describe, it, expect, beforeEach } from 'vitest';
import { useLogStore } from './logStore';
import type { LogEntry } from '@/types/stream';

// N3: sample store test. logStore is a dependency-free Zustand store;
// its load-bearing logic is the 1000-entry bounded buffer, which is
// off-by-one prone (`slice(-999)` + the new entry). These tests pin the
// cap and the FIFO drop order so a regression in the slice math — which
// would otherwise grow the buffer unboundedly and leak memory during a
// long stream — is caught.

function makeLog(n: number): LogEntry {
  return {
    id: String(n),
    timestamp: new Date(0),
    level: 'info',
    message: `log ${n}`,
  };
}

beforeEach(() => {
  useLogStore.setState({ logs: [], filter: 'all', autoScroll: true, timeFilter: 'all' });
});

describe('logStore.addLog', () => {
  it('appends a single entry', () => {
    useLogStore.getState().addLog(makeLog(1));
    expect(useLogStore.getState().logs).toHaveLength(1);
    expect(useLogStore.getState().logs[0].message).toBe('log 1');
  });

  it('caps the buffer at 1000 entries, dropping oldest first', () => {
    const { addLog } = useLogStore.getState();
    for (let i = 0; i <= 1000; i++) addLog(makeLog(i));
    const { logs } = useLogStore.getState();
    expect(logs).toHaveLength(1000);
    // Oldest (id "0") dropped; newest (id "1000") retained at the tail.
    expect(logs[0].id).toBe('1');
    expect(logs[logs.length - 1].id).toBe('1000');
  });
});

describe('logStore.addLogs', () => {
  it('caps a bulk insert at the most recent 1000', () => {
    const batch = Array.from({ length: 1500 }, (_, i) => makeLog(i));
    useLogStore.getState().addLogs(batch);
    const { logs } = useLogStore.getState();
    expect(logs).toHaveLength(1000);
    expect(logs[0].id).toBe('500');
    expect(logs[logs.length - 1].id).toBe('1499');
  });
});

describe('logStore UI state', () => {
  it('clearLogs empties the buffer', () => {
    useLogStore.getState().addLog(makeLog(1));
    useLogStore.getState().clearLogs();
    expect(useLogStore.getState().logs).toHaveLength(0);
  });

  it('setFilter / setTimeFilter / setAutoScroll update independently', () => {
    const s = useLogStore.getState();
    s.setFilter('error');
    s.setTimeFilter('1h');
    s.setAutoScroll(false);
    const next = useLogStore.getState();
    expect(next.filter).toBe('error');
    expect(next.timeFilter).toBe('1h');
    expect(next.autoScroll).toBe(false);
  });
});
