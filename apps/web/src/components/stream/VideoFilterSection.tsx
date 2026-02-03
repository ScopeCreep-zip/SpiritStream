/**
 * Video Filter Section
 * Collapsible section in Properties Panel for managing video filters on a layer
 */
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronDown, ChevronRight, Plus, Trash2, Settings, GripVertical } from 'lucide-react';
import {
  type VideoFilter,
  type VideoFilterType,
  VIDEO_FILTER_TYPES,
  getVideoFilterLabel,
  createChromaKeyFilter,
  createColorKeyFilter,
  createColorCorrectionFilter,
  createLUTFilter,
  createBlurFilter,
  createSharpenFilter,
  createScrollFilter,
  createMaskFilter,
  createTransform3DFilter,
} from '@/types/source';

interface VideoFilterSectionProps {
  layerId: string;
  filters: VideoFilter[];
  onFiltersChange: (filters: VideoFilter[]) => void;
}

export function VideoFilterSection({
  layerId: _layerId,
  filters,
  onFiltersChange,
}: VideoFilterSectionProps) {
  const { t } = useTranslation();
  const [isExpanded, setIsExpanded] = useState(true);
  const [showAddMenu, setShowAddMenu] = useState(false);
  const [editingFilterId, setEditingFilterId] = useState<string | null>(null);

  // _layerId is available for future use (e.g., persisting filter configs per layer)

  const handleAddFilter = (type: VideoFilterType) => {
    let newFilter: VideoFilter;
    switch (type) {
      case 'chromaKey':
        newFilter = createChromaKeyFilter();
        break;
      case 'colorKey':
        newFilter = createColorKeyFilter();
        break;
      case 'colorCorrection':
        newFilter = createColorCorrectionFilter();
        break;
      case 'lut':
        newFilter = createLUTFilter();
        break;
      case 'blur':
        newFilter = createBlurFilter();
        break;
      case 'sharpen':
        newFilter = createSharpenFilter();
        break;
      case 'scroll':
        newFilter = createScrollFilter();
        break;
      case 'mask':
        newFilter = createMaskFilter();
        break;
      case 'transform3d':
        newFilter = createTransform3DFilter();
        break;
    }
    newFilter.order = filters.length;
    onFiltersChange([...filters, newFilter]);
    setShowAddMenu(false);
  };

  const handleToggleFilter = (filterId: string) => {
    onFiltersChange(
      filters.map((f) =>
        f.id === filterId ? ({ ...f, enabled: !f.enabled } as VideoFilter) : f
      )
    );
  };

  const handleRemoveFilter = (filterId: string) => {
    onFiltersChange(filters.filter((f) => f.id !== filterId));
    if (editingFilterId === filterId) {
      setEditingFilterId(null);
    }
  };

  const handleFilterUpdate = (filterId: string, updates: Partial<VideoFilter>) => {
    onFiltersChange(
      filters.map((f) =>
        f.id === filterId ? ({ ...f, ...updates } as VideoFilter) : f
      )
    );
  };

  const activeFilterCount = filters.filter((f) => f.enabled).length;

  return (
    <div className="border-t border-[var(--border-default)]">
      {/* Section Header - using div with role="button" to allow nested button */}
      <div
        role="button"
        tabIndex={0}
        onClick={() => setIsExpanded(!isExpanded)}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            setIsExpanded(!isExpanded);
          }
        }}
        className="w-full flex items-center justify-between px-3 py-2 hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
      >
        <div className="flex items-center gap-2">
          {isExpanded ? (
            <ChevronDown className="w-4 h-4 text-[var(--text-muted)]" />
          ) : (
            <ChevronRight className="w-4 h-4 text-[var(--text-muted)]" />
          )}
          <span className="text-xs font-medium text-[var(--text-primary)]">
            {t('video.filters', { defaultValue: 'Video Filters' })}
          </span>
          {activeFilterCount > 0 && (
            <span className="px-1.5 py-0.5 text-[10px] font-medium bg-primary/20 text-primary rounded">
              {activeFilterCount}
            </span>
          )}
        </div>
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            setShowAddMenu(!showAddMenu);
          }}
          className="p-1 rounded hover:bg-[var(--bg-elevated)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors"
        >
          <Plus className="w-3.5 h-3.5" />
        </button>
      </div>

      {/* Add Filter Menu */}
      {showAddMenu && (
        <div className="mx-3 mb-2 bg-[var(--bg-surface)] border border-[var(--border-default)] rounded-lg shadow-lg py-1 z-10">
          {VIDEO_FILTER_TYPES.map((type) => (
            <button
              key={type}
              type="button"
              onClick={() => handleAddFilter(type)}
              className="w-full px-3 py-1.5 text-left text-xs text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
            >
              {getVideoFilterLabel(type)}
            </button>
          ))}
        </div>
      )}

      {/* Filters List */}
      {isExpanded && (
        <div className="px-3 pb-3">
          {filters.length === 0 ? (
            <div className="text-center text-xs text-[var(--text-muted)] py-3">
              {t('video.noFilters', { defaultValue: 'No filters applied' })}
            </div>
          ) : (
            <div className="space-y-1">
              {filters
                .sort((a, b) => a.order - b.order)
                .map((filter) => (
                  <div key={filter.id}>
                    <div className="flex items-center gap-2 p-2 rounded bg-[var(--bg-sunken)] group">
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
                        {getVideoFilterLabel(filter.type)}
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

                    {/* Inline settings when editing */}
                    {editingFilterId === filter.id && (
                      <VideoFilterSettings
                        filter={filter}
                        onUpdate={(updates) => handleFilterUpdate(filter.id, updates)}
                      />
                    )}
                  </div>
                ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

interface VideoFilterSettingsProps {
  filter: VideoFilter;
  onUpdate: (updates: Partial<VideoFilter>) => void;
}

function VideoFilterSettings({ filter, onUpdate }: VideoFilterSettingsProps) {
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

  const renderColorPicker = (
    label: string,
    value: string,
    onChange: (value: string) => void
  ) => (
    <div className="flex items-center justify-between">
      <span className="text-[10px] text-[var(--text-muted)]">{label}</span>
      <div className="flex items-center gap-2">
        <input
          type="color"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="w-6 h-6 rounded cursor-pointer border border-[var(--border-default)]"
        />
        <span className="text-[10px] text-[var(--text-secondary)] font-mono uppercase">
          {value}
        </span>
      </div>
    </div>
  );

  return (
    <div className="mt-1 p-3 rounded bg-[var(--bg-base)] border border-[var(--border-default)] space-y-3">
      <h5 className="text-[10px] font-medium text-[var(--text-muted)] uppercase">
        {getVideoFilterLabel(filter.type)} {t('common.settings', { defaultValue: 'Settings' })}
      </h5>

      {/* Chroma Key */}
      {filter.type === 'chromaKey' && (
        <>
          {renderColorPicker(
            t('video.keyColor', { defaultValue: 'Key Color' }),
            filter.keyColor,
            (v) => onUpdate({ keyColor: v })
          )}
          {renderSlider(
            t('video.similarity', { defaultValue: 'Similarity' }),
            filter.similarity,
            0,
            1000,
            10,
            '',
            (v) => onUpdate({ similarity: v })
          )}
          {renderSlider(
            t('video.smoothness', { defaultValue: 'Smoothness' }),
            filter.smoothness,
            0,
            1000,
            10,
            '',
            (v) => onUpdate({ smoothness: v })
          )}
          {renderSlider(
            t('video.keySpill', { defaultValue: 'Key Spill' }),
            filter.keySpill,
            0,
            1000,
            10,
            '',
            (v) => onUpdate({ keySpill: v })
          )}
        </>
      )}

      {/* Color Key */}
      {filter.type === 'colorKey' && (
        <>
          {renderColorPicker(
            t('video.keyColor', { defaultValue: 'Key Color' }),
            filter.keyColor,
            (v) => onUpdate({ keyColor: v })
          )}
          {renderSlider(
            t('video.similarity', { defaultValue: 'Similarity' }),
            filter.similarity,
            0,
            1000,
            10,
            '',
            (v) => onUpdate({ similarity: v })
          )}
          {renderSlider(
            t('video.smoothness', { defaultValue: 'Smoothness' }),
            filter.smoothness,
            0,
            1000,
            10,
            '',
            (v) => onUpdate({ smoothness: v })
          )}
        </>
      )}

      {/* Color Correction */}
      {filter.type === 'colorCorrection' && (
        <>
          {renderSlider(
            t('video.brightness', { defaultValue: 'Brightness' }),
            filter.brightness,
            -1,
            1,
            0.01,
            '',
            (v) => onUpdate({ brightness: v })
          )}
          {renderSlider(
            t('video.contrast', { defaultValue: 'Contrast' }),
            filter.contrast,
            -1,
            1,
            0.01,
            '',
            (v) => onUpdate({ contrast: v })
          )}
          {renderSlider(
            t('video.saturation', { defaultValue: 'Saturation' }),
            filter.saturation,
            0,
            3,
            0.01,
            '',
            (v) => onUpdate({ saturation: v })
          )}
          {renderSlider(
            t('video.gamma', { defaultValue: 'Gamma' }),
            filter.gamma,
            0.1,
            4,
            0.1,
            '',
            (v) => onUpdate({ gamma: v })
          )}
          {renderSlider(
            t('video.hue', { defaultValue: 'Hue' }),
            filter.hue,
            -180,
            180,
            1,
            '°',
            (v) => onUpdate({ hue: v })
          )}
        </>
      )}

      {/* LUT */}
      {filter.type === 'lut' && (
        <>
          <div className="space-y-1">
            <label className="text-[10px] text-[var(--text-muted)]">
              {t('video.lutFile', { defaultValue: 'LUT File' })}
            </label>
            <input
              type="text"
              value={filter.lutFile}
              onChange={(e) => onUpdate({ lutFile: e.target.value })}
              placeholder="Path to .cube or .3dl file"
              className="w-full px-2 py-1 text-xs bg-[var(--bg-sunken)] border border-[var(--border-default)] rounded text-[var(--text-primary)] placeholder-[var(--text-muted)]"
            />
          </div>
          {renderSlider(
            t('video.intensity', { defaultValue: 'Intensity' }),
            filter.intensity,
            0,
            1,
            0.01,
            '',
            (v) => onUpdate({ intensity: v })
          )}
        </>
      )}

      {/* Blur */}
      {filter.type === 'blur' && (
        <>
          <div className="space-y-1">
            <label className="text-[10px] text-[var(--text-muted)]">
              {t('video.blurType', { defaultValue: 'Blur Type' })}
            </label>
            <select
              value={filter.blurType}
              onChange={(e) => onUpdate({ blurType: e.target.value as 'box' | 'gaussian' })}
              className="w-full px-2 py-1 text-xs bg-[var(--bg-sunken)] border border-[var(--border-default)] rounded text-[var(--text-primary)]"
            >
              <option value="gaussian">{t('video.gaussian', { defaultValue: 'Gaussian' })}</option>
              <option value="box">{t('video.box', { defaultValue: 'Box' })}</option>
            </select>
          </div>
          {renderSlider(
            t('video.size', { defaultValue: 'Size' }),
            filter.size,
            1,
            100,
            1,
            'px',
            (v) => onUpdate({ size: v })
          )}
        </>
      )}

      {/* Sharpen */}
      {filter.type === 'sharpen' && (
        <>
          {renderSlider(
            t('video.amount', { defaultValue: 'Amount' }),
            filter.amount,
            0,
            10,
            0.1,
            '',
            (v) => onUpdate({ amount: v })
          )}
        </>
      )}

      {/* Scroll */}
      {filter.type === 'scroll' && (
        <>
          {renderSlider(
            t('video.horizontalSpeed', { defaultValue: 'Horizontal Speed' }),
            filter.horizontalSpeed,
            -1000,
            1000,
            10,
            ' px/s',
            (v) => onUpdate({ horizontalSpeed: v })
          )}
          {renderSlider(
            t('video.verticalSpeed', { defaultValue: 'Vertical Speed' }),
            filter.verticalSpeed,
            -1000,
            1000,
            10,
            ' px/s',
            (v) => onUpdate({ verticalSpeed: v })
          )}
          <label className="flex items-center gap-2 text-[10px] text-[var(--text-muted)]">
            <input
              type="checkbox"
              checked={filter.loop}
              onChange={(e) => onUpdate({ loop: e.target.checked })}
              className="h-3.5 w-3.5 rounded border-[var(--border-default)] bg-[var(--bg-base)] text-primary focus:ring-primary/50 cursor-pointer"
            />
            {t('video.loop', { defaultValue: 'Loop' })}
          </label>
        </>
      )}

      {/* Mask */}
      {filter.type === 'mask' && (
        <>
          <div className="space-y-1">
            <label className="text-[10px] text-[var(--text-muted)]">
              {t('video.maskImage', { defaultValue: 'Mask Image' })}
            </label>
            <input
              type="text"
              value={filter.maskImage}
              onChange={(e) => onUpdate({ maskImage: e.target.value })}
              placeholder="Path to mask image"
              className="w-full px-2 py-1 text-xs bg-[var(--bg-sunken)] border border-[var(--border-default)] rounded text-[var(--text-primary)] placeholder-[var(--text-muted)]"
            />
          </div>
          <div className="space-y-1">
            <label className="text-[10px] text-[var(--text-muted)]">
              {t('video.maskType', { defaultValue: 'Mask Type' })}
            </label>
            <select
              value={filter.maskType}
              onChange={(e) => onUpdate({ maskType: e.target.value as 'alpha' | 'luminance' })}
              className="w-full px-2 py-1 text-xs bg-[var(--bg-sunken)] border border-[var(--border-default)] rounded text-[var(--text-primary)]"
            >
              <option value="alpha">{t('video.alpha', { defaultValue: 'Alpha' })}</option>
              <option value="luminance">{t('video.luminance', { defaultValue: 'Luminance' })}</option>
            </select>
          </div>
          <label className="flex items-center gap-2 text-[10px] text-[var(--text-muted)]">
            <input
              type="checkbox"
              checked={filter.invert}
              onChange={(e) => onUpdate({ invert: e.target.checked })}
              className="h-3.5 w-3.5 rounded border-[var(--border-default)] bg-[var(--bg-base)] text-primary focus:ring-primary/50 cursor-pointer"
            />
            {t('video.invert', { defaultValue: 'Invert' })}
          </label>
        </>
      )}

      {/* 3D Transform */}
      {filter.type === 'transform3d' && (
        <>
          {renderSlider(
            t('video.rotationX', { defaultValue: 'Rotation X' }),
            filter.rotationX,
            -180,
            180,
            1,
            '°',
            (v) => onUpdate({ rotationX: v })
          )}
          {renderSlider(
            t('video.rotationY', { defaultValue: 'Rotation Y' }),
            filter.rotationY,
            -180,
            180,
            1,
            '°',
            (v) => onUpdate({ rotationY: v })
          )}
          {renderSlider(
            t('video.rotationZ', { defaultValue: 'Rotation Z' }),
            filter.rotationZ,
            -180,
            180,
            1,
            '°',
            (v) => onUpdate({ rotationZ: v })
          )}
          {renderSlider(
            t('video.perspective', { defaultValue: 'Perspective' }),
            filter.perspective,
            100,
            5000,
            50,
            'px',
            (v) => onUpdate({ perspective: v })
          )}
          {renderSlider(
            t('video.positionX', { defaultValue: 'Position X' }),
            filter.positionX,
            -500,
            500,
            5,
            'px',
            (v) => onUpdate({ positionX: v })
          )}
          {renderSlider(
            t('video.positionY', { defaultValue: 'Position Y' }),
            filter.positionY,
            -500,
            500,
            5,
            'px',
            (v) => onUpdate({ positionY: v })
          )}
          {renderSlider(
            t('video.positionZ', { defaultValue: 'Position Z' }),
            filter.positionZ,
            -500,
            500,
            5,
            'px',
            (v) => onUpdate({ positionZ: v })
          )}
        </>
      )}
    </div>
  );
}
