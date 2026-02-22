/**
 * useSourceLookup
 *
 * Replaces the repeated `useMemo(() => new Map(...))` pattern for O(1)
 * source-by-id lookup from the current profile.
 */
import { useMemo } from 'react';
import { useProfileStore } from '@/stores/profileStore';
import type { Source } from '@/types/source';

/** O(1) source-by-id lookup from the current profile's sources */
export function useSourceLookup(): Map<string, Source> {
  const sources = useProfileStore((s) => s.current?.sources ?? []);
  return useMemo(() => new Map(sources.map((s) => [s.id, s])), [sources]);
}
