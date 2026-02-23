/**
 * FilterPanel — Collapsible panel for a single audio filter
 * Extracted from AudioFilterChainEditor
 */
import { ChevronDown, ChevronRight, Trash2 } from 'lucide-react';
import type { AudioFilter } from '@/types/source';
import { getAudioFilterLabel } from '@/types/source';
import { FilterParams } from './FilterParamEditors';

interface FilterPanelProps {
  filter: AudioFilter;
  expanded: boolean;
  onToggleExpanded: () => void;
  onToggleEnabled: () => void;
  onUpdate: (updates: Partial<AudioFilter>) => void;
  onRemove: () => void;
}

export function FilterPanel({ filter, expanded, onToggleExpanded, onToggleEnabled, onUpdate, onRemove }: FilterPanelProps) {
  return (
    <div className={`border rounded-md ${filter.enabled ? 'border-[var(--border-default)]' : 'border-[var(--border-subtle)] opacity-60'}`}>
      {/* Header */}
      <div className="flex items-center gap-2 px-2 py-1.5 cursor-pointer" onClick={onToggleExpanded}>
        {expanded ? <ChevronDown className="w-3 h-3 text-[var(--text-muted)]" /> : <ChevronRight className="w-3 h-3 text-[var(--text-muted)]" />}
        <input
          type="checkbox"
          checked={filter.enabled}
          onChange={(e) => { e.stopPropagation(); onToggleEnabled(); }}
          className="w-3 h-3 accent-[var(--primary)]"
        />
        <span className="text-[11px] font-medium text-[var(--text-secondary)] flex-1">
          {getAudioFilterLabel(filter.type)}
        </span>
        <button
          type="button"
          className="text-[var(--text-muted)] hover:text-red-400 transition-colors"
          onClick={(e) => { e.stopPropagation(); onRemove(); }}
          title="Remove filter"
        >
          <Trash2 className="w-3 h-3" />
        </button>
      </div>

      {/* Params */}
      {expanded && (
        <div className="px-3 pb-2 pt-1 border-t border-[var(--border-subtle)]">
          <FilterParams filter={filter} onUpdate={onUpdate} />
        </div>
      )}
    </div>
  );
}
