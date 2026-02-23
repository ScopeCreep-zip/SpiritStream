/**
 * Audio Filter Button
 * Button with popover for managing audio filters on a track
 */
import { useState, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { SlidersHorizontal, Plus, Trash2, GripVertical, Settings } from 'lucide-react';
import {
  type AudioFilter,
  type AudioFilterType,
  type Source,
  AUDIO_FILTER_TYPES,
  getAudioFilterLabel,
  createCompressorFilter,
  createNoiseGateFilter,
  createNoiseSuppressionFilter,
  createGainFilter,
  createExpanderFilter,
  sourceHasAudio,
} from '@/types/source';
import { AudioFilterSettings } from './audio/AudioFilterSettings';

interface AudioFilterButtonProps {
  trackId: string;
  trackName: string;
  filters: AudioFilter[];
  onFiltersChange: (filters: AudioFilter[]) => void;
  /** Available sources for sidechain selection */
  availableSources?: Source[];
  /** Use compact size to match M/S buttons in channel strip */
  compact?: boolean;
}

export function AudioFilterButton({
  trackId,
  trackName,
  filters,
  onFiltersChange,
  availableSources = [],
  compact = false,
}: AudioFilterButtonProps) {
  const { t } = useTranslation();

  // Filter available sources to only those with audio (for sidechain options)
  // Exclude the current track's source from sidechain options
  const sidechainOptions = availableSources.filter(
    (s) => sourceHasAudio(s) && s.id !== trackId
  );
  const [isOpen, setIsOpen] = useState(false);
  const [showAddMenu, setShowAddMenu] = useState(false);
  const [editingFilterId, setEditingFilterId] = useState<string | null>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);

  const hasActiveFilters = filters.some((f) => f.enabled);

  // Close popover when clicking outside
  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (
        popoverRef.current &&
        !popoverRef.current.contains(event.target as Node) &&
        buttonRef.current &&
        !buttonRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false);
        setShowAddMenu(false);
        setEditingFilterId(null);
      }
    }

    if (isOpen) {
      document.addEventListener('mousedown', handleClickOutside);
      return () => document.removeEventListener('mousedown', handleClickOutside);
    }
  }, [isOpen]);

  const handleAddFilter = (type: AudioFilterType) => {
    let newFilter: AudioFilter;
    switch (type) {
      case 'compressor':
        newFilter = createCompressorFilter();
        break;
      case 'noiseGate':
        newFilter = createNoiseGateFilter();
        break;
      case 'noiseSuppression':
        newFilter = createNoiseSuppressionFilter();
        break;
      case 'gain':
        newFilter = createGainFilter();
        break;
      case 'expander':
        newFilter = createExpanderFilter();
        break;
    }
    newFilter.order = filters.length;
    onFiltersChange([...filters, newFilter]);
    setShowAddMenu(false);
  };

  const handleToggleFilter = (filterId: string) => {
    onFiltersChange(
      filters.map((f) =>
        f.id === filterId ? ({ ...f, enabled: !f.enabled } as AudioFilter) : f
      )
    );
  };

  const handleRemoveFilter = (filterId: string) => {
    onFiltersChange(filters.filter((f) => f.id !== filterId));
  };

  const handleFilterUpdate = (filterId: string, updates: Partial<AudioFilter>) => {
    onFiltersChange(
      filters.map((f) =>
        f.id === filterId ? ({ ...f, ...updates } as AudioFilter) : f
      )
    );
  };

  return (
    <div className="relative">
      <button
        ref={buttonRef}
        onClick={() => setIsOpen(!isOpen)}
        className={`${compact ? 'w-5 h-5 rounded' : 'w-7 h-7 rounded-md'} flex items-center justify-center transition-colors border ${
          hasActiveFilters
            ? 'bg-primary/20 border-primary text-primary'
            : 'bg-[var(--bg-sunken)] border-transparent text-[var(--text-muted)] hover:bg-[var(--bg-elevated)] hover:text-[var(--text-secondary)]'
        }`}
        title={t('audio.filters', { defaultValue: 'Audio Filters' })}
      >
        <SlidersHorizontal className={compact ? 'w-2.5 h-2.5' : 'w-4 h-4'} />
      </button>

      {isOpen && (
        <div
          ref={popoverRef}
          className="absolute bottom-full mb-1 left-1/2 -translate-x-1/2 z-50 w-64 bg-[var(--bg-elevated)] border border-[var(--border-default)] rounded-lg shadow-lg"
        >
          <div className="p-3 border-b border-[var(--border-default)]">
            <h4 className="text-xs font-medium text-[var(--text-primary)]">
              {t('audio.filtersFor', { name: trackName, defaultValue: `Filters - ${trackName}` })}
            </h4>
          </div>

          <div className="max-h-60 overflow-y-auto">
            {filters.length === 0 ? (
              <div className="p-4 text-center text-xs text-[var(--text-muted)]">
                {t('audio.noFilters', { defaultValue: 'No filters applied' })}
              </div>
            ) : (
              <div className="p-2 space-y-1">
                {filters
                  .sort((a, b) => a.order - b.order)
                  .map((filter) => (
                    <div
                      key={filter.id}
                      className="flex items-center gap-2 p-2 rounded bg-[var(--bg-sunken)] group"
                    >
                      <GripVertical className="w-3 h-3 text-[var(--text-muted)] cursor-grab" />
                      <input
                        type="checkbox"
                        checked={filter.enabled}
                        onChange={() => handleToggleFilter(filter.id)}
                        className="h-3.5 w-3.5 rounded border-[var(--border-default)] bg-[var(--bg-base)] text-primary focus:ring-primary/50 cursor-pointer"
                      />
                      <span
                        className={`flex-1 text-xs ${
                          filter.enabled
                            ? 'text-[var(--text-secondary)]'
                            : 'text-[var(--text-muted)]'
                        }`}
                      >
                        {getAudioFilterLabel(filter.type)}
                      </span>
                      <button
                        type="button"
                        onClick={() =>
                          setEditingFilterId(
                            editingFilterId === filter.id ? null : filter.id
                          )
                        }
                        className="p-1 rounded hover:bg-[var(--bg-hover)] text-[var(--text-muted)] hover:text-[var(--text-secondary)] opacity-0 group-hover:opacity-100 transition-opacity"
                      >
                        <Settings className="w-3 h-3" />
                      </button>
                      <button
                        type="button"
                        onClick={() => handleRemoveFilter(filter.id)}
                        className="p-1 rounded hover:bg-destructive/20 text-[var(--text-muted)] hover:text-destructive opacity-0 group-hover:opacity-100 transition-opacity"
                      >
                        <Trash2 className="w-3 h-3" />
                      </button>
                    </div>
                  ))}
              </div>
            )}
          </div>

          {/* Filter settings panel (inline) */}
          {editingFilterId && (
            <AudioFilterSettings
              filter={filters.find((f) => f.id === editingFilterId)!}
              onUpdate={(updates) => handleFilterUpdate(editingFilterId, updates)}
              sidechainOptions={sidechainOptions}
            />
          )}

          {/* Add filter button */}
          <div className="p-2 border-t border-[var(--border-default)]">
            <div className="relative">
              <button
                type="button"
                onClick={() => setShowAddMenu(!showAddMenu)}
                className="w-full flex items-center justify-center gap-1 px-2 py-1.5 text-xs text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] rounded transition-colors"
              >
                <Plus className="w-3 h-3" />
                {t('audio.addFilter', { defaultValue: 'Add Filter' })}
              </button>

              {showAddMenu && (
                <div className="absolute bottom-full mb-1 left-0 right-0 bg-[var(--bg-surface)] border border-[var(--border-default)] rounded-lg shadow-lg py-1 z-10">
                  {AUDIO_FILTER_TYPES.map((type) => (
                    <button
                      key={type}
                      type="button"
                      onClick={() => handleAddFilter(type)}
                      className="w-full px-3 py-1.5 text-left text-xs text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                    >
                      {getAudioFilterLabel(type)}
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

