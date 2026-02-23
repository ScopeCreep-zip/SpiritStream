/**
 * Audio Filter Chain Editor
 * Real filter parameter editor with collapsible panels per filter.
 * Sends changes to POST /api/audio/filters/{sourceId}.
 */
import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import type { AudioFilter, AudioFilterType } from '@/types/source';
import { createAudioFilter } from '@/types/source';
import { AddFilterDropdown } from './audio/AddFilterDropdown';
import { FilterPanel } from './audio/FilterPanel';

interface AudioFilterChainEditorProps {
  filters: AudioFilter[];
  onFiltersChange: (filters: AudioFilter[]) => void;
}

export function AudioFilterChainEditor({
  filters,
  onFiltersChange,
}: AudioFilterChainEditorProps) {
  const { t } = useTranslation();
  const [expandedFilter, setExpandedFilter] = useState<string | null>(null);

  const handleAddFilter = useCallback((type: AudioFilterType) => {
    const newFilter = createAudioFilter(type);
    const updated = [...filters, newFilter];
    onFiltersChange(updated);
    setExpandedFilter(newFilter.id);
  }, [filters, onFiltersChange]);

  const handleRemoveFilter = useCallback((filterId: string) => {
    const updated = filters.filter(f => f.id !== filterId);
    onFiltersChange(updated);
  }, [filters, onFiltersChange]);

  const handleUpdateFilter = useCallback((filterId: string, updates: Partial<AudioFilter>) => {
    const updated = filters.map(f =>
      f.id === filterId ? { ...f, ...updates } as AudioFilter : f
    );
    onFiltersChange(updated);
  }, [filters, onFiltersChange]);

  const handleToggleEnabled = useCallback((filterId: string) => {
    const filter = filters.find(f => f.id === filterId);
    if (filter) {
      handleUpdateFilter(filterId, { enabled: !filter.enabled });
    }
  }, [filters, handleUpdateFilter]);

  const toggleExpanded = useCallback((filterId: string) => {
    setExpandedFilter(prev => prev === filterId ? null : filterId);
  }, []);

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <h4 className="text-xs font-medium text-[var(--text-secondary)]">
          {t('audio.filters', { defaultValue: 'Audio Filters' })}
        </h4>
        <AddFilterDropdown onAdd={handleAddFilter} />
      </div>

      {filters.length === 0 ? (
        <p className="text-[10px] text-[var(--text-muted)] py-2">
          {t('audio.noFilters', { defaultValue: 'No audio filters applied.' })}
        </p>
      ) : (
        <div className="flex flex-col gap-1">
          {filters.map((filter) => (
            <FilterPanel
              key={filter.id}
              filter={filter}
              expanded={expandedFilter === filter.id}
              onToggleExpanded={() => toggleExpanded(filter.id)}
              onToggleEnabled={() => handleToggleEnabled(filter.id)}
              onUpdate={(updates) => handleUpdateFilter(filter.id, updates)}
              onRemove={() => handleRemoveFilter(filter.id)}
            />
          ))}
        </div>
      )}
    </div>
  );
}
