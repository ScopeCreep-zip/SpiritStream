import { create } from 'zustand';
import type { LogLevel, LogEntry } from '@/types/stream';

interface LogStore {
  logs: LogEntry[];
  filter: LogLevel | 'all';
  autoScroll: boolean;
  timeFilter: 'all' | '15m' | '1h' | '24h' | '7d';

  // Actions
  addLog: (log: LogEntry) => void;
  addLogs: (logs: LogEntry[]) => void;
  setFilter: (filter: LogLevel | 'all') => void;
  setAutoScroll: (enabled: boolean) => void;
  setTimeFilter: (filter: 'all' | '15m' | '1h' | '24h' | '7d') => void;
  clearLogs: () => void;
}

export const useLogStore = create<LogStore>((set) => ({
  logs: [],
  filter: 'all',
  autoScroll: true,
  timeFilter: 'all',

  addLog: (log) =>
    set((state) => ({
      // Keep last 1000 logs
      logs: [...state.logs.slice(-999), log],
    })),

  addLogs: (logs) =>
    set((state) => ({
      logs: [...state.logs, ...logs].slice(-1000),
    })),

  setFilter: (filter) => set({ filter }),

  setAutoScroll: (enabled) => set({ autoScroll: enabled }),

  setTimeFilter: (filter) => set({ timeFilter: filter }),

  clearLogs: () => set({ logs: [] }),
}));
