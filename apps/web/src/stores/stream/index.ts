/// Composed Zustand store for stream state.
///
/// Slices live in sibling files (`core.ts`, `stats.ts`) and are merged
/// here. Each slice's `get()` returns the full combined state so
/// cross-slice calls (e.g. core.startGroup → stats.setGlobalStatus)
/// keep working.

import { create } from 'zustand';
import { createCoreSlice } from './core';
import { createStatsSlice } from './stats';
import type { StreamState } from './types';

export type { FFmpegStats, GroupStats, StreamState } from './types';

export const useStreamStore = create<StreamState>()((...args) => ({
  ...createCoreSlice(...args),
  ...createStatsSlice(...args),
}));
