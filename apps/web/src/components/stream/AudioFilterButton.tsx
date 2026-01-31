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

interface AudioFilterButtonProps {
  trackId: string;
  trackName: string;
  filters: AudioFilter[];
  onFiltersChange: (filters: AudioFilter[]) => void;
  /** Available sources for sidechain selection */
  availableSources?: Source[];
}

export function AudioFilterButton({
  trackId,
  trackName,
  filters,
  onFiltersChange,
  availableSources = [],
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
        className={`w-7 h-7 rounded-md flex items-center justify-center transition-all border ${
          hasActiveFilters
            ? 'bg-primary/20 border-primary text-primary'
            : 'bg-[var(--bg-sunken)] border-transparent text-[var(--text-muted)] hover:bg-[var(--bg-elevated)] hover:text-[var(--text-secondary)]'
        }`}
        title={t('audio.filters', { defaultValue: 'Audio Filters' })}
      >
        <SlidersHorizontal className="w-4 h-4" />
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

interface AudioFilterSettingsProps {
  filter: AudioFilter;
  onUpdate: (updates: Partial<AudioFilter>) => void;
  /** Available sources for sidechain selection (compressor only) */
  sidechainOptions?: Source[];
}

function AudioFilterSettings({ filter, onUpdate, sidechainOptions = [] }: AudioFilterSettingsProps) {
  const { t } = useTranslation();

  const renderSlider = (
    label: string,
    value: number,
    min: number,
    max: number,
    step: number,
    unit: string,
    onChange: (value: number) => void
  ) => (
    <div className="space-y-1">
      <div className="flex justify-between text-[10px]">
        <span className="text-[var(--text-muted)]">{label}</span>
        <span className="text-[var(--text-secondary)] tabular-nums">
          {value}
          {unit}
        </span>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(parseFloat(e.target.value))}
        className="w-full h-1.5 bg-[var(--bg-sunken)] rounded-full appearance-none cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-primary"
      />
    </div>
  );

  return (
    <div className="p-3 border-t border-[var(--border-default)] bg-[var(--bg-sunken)] space-y-3">
      <h5 className="text-[10px] font-medium text-[var(--text-muted)] uppercase">
        {getAudioFilterLabel(filter.type)} {t('common.settings', { defaultValue: 'Settings' })}
      </h5>

      {filter.type === 'gain' &&
        renderSlider(
          t('audio.gain', { defaultValue: 'Gain' }),
          filter.gain,
          -30,
          30,
          0.5,
          ' dB',
          (v) => onUpdate({ gain: v })
        )}

      {filter.type === 'compressor' && (
        <>
          {renderSlider(
            t('audio.threshold', { defaultValue: 'Threshold' }),
            filter.threshold,
            -60,
            0,
            1,
            ' dB',
            (v) => onUpdate({ threshold: v })
          )}
          {renderSlider(
            t('audio.ratio', { defaultValue: 'Ratio' }),
            filter.ratio,
            1,
            32,
            0.5,
            ':1',
            (v) => onUpdate({ ratio: v })
          )}
          {renderSlider(
            t('audio.attack', { defaultValue: 'Attack' }),
            filter.attack,
            0,
            500,
            1,
            ' ms',
            (v) => onUpdate({ attack: v })
          )}
          {renderSlider(
            t('audio.release', { defaultValue: 'Release' }),
            filter.release,
            0,
            1000,
            1,
            ' ms',
            (v) => onUpdate({ release: v })
          )}
          {renderSlider(
            t('audio.outputGain', { defaultValue: 'Output Gain' }),
            filter.outputGain,
            -30,
            30,
            0.5,
            ' dB',
            (v) => onUpdate({ outputGain: v })
          )}

          {/* Sidechain Source Selector for Audio Ducking */}
          <div className="space-y-1 pt-2 border-t border-[var(--border-muted)]">
            <div className="flex justify-between text-[10px]">
              <span className="text-[var(--text-muted)]">
                {t('audio.sidechainSource', { defaultValue: 'Sidechain Source' })}
              </span>
            </div>
            <select
              value={filter.sidechainSourceId || ''}
              onChange={(e) => onUpdate({ sidechainSourceId: e.target.value || undefined })}
              className="w-full h-6 px-1.5 text-[10px] bg-[var(--bg-base)] border border-[var(--border-default)] rounded text-[var(--text-secondary)] focus:outline-none focus:ring-1 focus:ring-primary/50"
            >
              <option value="">
                {t('audio.noSidechain', { defaultValue: 'None (no ducking)' })}
              </option>
              {sidechainOptions.map((source) => (
                <option key={source.id} value={source.id}>
                  {source.name}
                </option>
              ))}
            </select>
            <p className="text-[9px] text-[var(--text-muted)] leading-tight">
              {t('audio.sidechainHelp', {
                defaultValue: 'When sidechain source is active, this track will be ducked (audio ducking).',
              })}
            </p>
          </div>
        </>
      )}

      {filter.type === 'noiseGate' && (
        <>
          {renderSlider(
            t('audio.threshold', { defaultValue: 'Threshold' }),
            filter.threshold,
            -60,
            0,
            1,
            ' dB',
            (v) => onUpdate({ threshold: v })
          )}
          {renderSlider(
            t('audio.attack', { defaultValue: 'Attack' }),
            filter.attack,
            0,
            100,
            1,
            ' ms',
            (v) => onUpdate({ attack: v })
          )}
          {renderSlider(
            t('audio.hold', { defaultValue: 'Hold' }),
            filter.hold,
            0,
            500,
            1,
            ' ms',
            (v) => onUpdate({ hold: v })
          )}
          {renderSlider(
            t('audio.release', { defaultValue: 'Release' }),
            filter.release,
            0,
            1000,
            1,
            ' ms',
            (v) => onUpdate({ release: v })
          )}
        </>
      )}

      {filter.type === 'noiseSuppression' &&
        renderSlider(
          t('audio.suppressionLevel', { defaultValue: 'Suppression Level' }),
          filter.level,
          0,
          100,
          1,
          '%',
          (v) => onUpdate({ level: v })
        )}

      {filter.type === 'expander' && (
        <>
          {renderSlider(
            t('audio.threshold', { defaultValue: 'Threshold' }),
            filter.threshold,
            -60,
            0,
            1,
            ' dB',
            (v) => onUpdate({ threshold: v })
          )}
          {renderSlider(
            t('audio.ratio', { defaultValue: 'Ratio' }),
            filter.ratio,
            1,
            10,
            0.5,
            ':1',
            (v) => onUpdate({ ratio: v })
          )}
          {renderSlider(
            t('audio.attack', { defaultValue: 'Attack' }),
            filter.attack,
            0,
            100,
            1,
            ' ms',
            (v) => onUpdate({ attack: v })
          )}
          {renderSlider(
            t('audio.release', { defaultValue: 'Release' }),
            filter.release,
            0,
            500,
            1,
            ' ms',
            (v) => onUpdate({ release: v })
          )}
        </>
      )}
    </div>
  );
}
