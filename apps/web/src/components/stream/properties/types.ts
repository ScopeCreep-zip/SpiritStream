/**
 * Shared types for property editor sub-components
 */
import type { TFunction } from 'i18next';
import type { Source } from '@/types/source';

export interface SourceEditorProps<S extends Source = Source> {
  source: S;
  profileName: string;
  updateSource: (profileName: string, sourceId: string, updates: Partial<Source>) => Promise<Source>;
  updateCurrentSource: (source: Source) => void;
  t: TFunction;
}
