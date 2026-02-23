/**
 * Unified Channel Strip
 * Combined VU meter + volume control in a single vertical bar
 *
 * Canvas rendering is handled by the meter coordinator (single RAF loop).
 * This component registers its canvas on mount and unregisters on unmount.
 */
import React, { useRef, useEffect, useState, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Headphones } from 'lucide-react';
import { AudioFilterButton } from './AudioFilterButton';
import type { AudioFilter, Source, MonitoringType } from '@/types/source';
import { linearToDb, dbToLinear } from '@/hooks/useAudioLevels';
import { formatDb } from '@/utils/formatters';
import {
  LABEL_WIDTH,
  BAR_WIDTH,
  ARROW_WIDTH,
  PADDING_Y,
  METER_HEIGHT,
  TOTAL_HEIGHT,
  TOTAL_WIDTH,
} from '@/lib/audio/meterRenderer';
import { registerMeter, unregisterMeter, type MeterConfig } from '@/lib/audio/meterCoordinator';

// Pre-computed layout values from meter constants (avoids repeated arithmetic in JSX)
const METER_LAYOUT = {
  labelOffset: LABEL_WIDTH + 8,
  stripWidth: TOTAL_WIDTH + 16,
  maxControlWidth: BAR_WIDTH + ARROW_WIDTH,
} as const;

export interface UnifiedChannelStripProps {
  trackId?: string;
  label: string;
  volume: number;
  muted: boolean;
  solo: boolean;
  filters?: AudioFilter[];
  monitoringType?: MonitoringType;
  balance?: number;
  isMaster?: boolean;
  availableSources?: Source[];
  captureError?: string;
  onVolumeChange: (volume: number) => void;
  onMuteToggle: (muted: boolean) => void;
  onSoloToggle: (solo: boolean) => void;
  onFiltersChange?: (filters: AudioFilter[]) => void;
}

export const UnifiedChannelStrip = React.memo(function UnifiedChannelStrip({
  trackId,
  label,
  volume,
  muted,
  solo,
  filters = [],
  monitoringType,
  balance,
  isMaster = false,
  availableSources = [],
  captureError,
  onVolumeChange,
  onMuteToggle,
  onSoloToggle,
  onFiltersChange,
}: UnifiedChannelStripProps) {
  const { t } = useTranslation();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const [localVolume, setLocalVolume] = useState(volume);
  const [isDragging, setIsDragging] = useState(false);
  const clipIndicatorRef = useRef<HTMLDivElement>(null);

  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const recentlyCommittedRef = useRef(false);
  const commitTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const peakDbRef = useRef<HTMLSpanElement>(null);

  // Memoize threshold filters to avoid allocating new arrays every render/frame
  const thresholdFilters = useMemo(
    () => filters.filter(f => ['noiseGate', 'compressor', 'expander'].includes(f.type)),
    [filters]
  );

  // Config ref — always fresh, read by coordinator's getConfig()
  const configRef = useRef<MeterConfig>({ volume: localVolume, muted, isDragging: false, thresholdFilters });
  configRef.current = { volume: localVolume, muted, isDragging, thresholdFilters };

  // Sync local volume from props
  useEffect(() => {
    if (!isDragging && !recentlyCommittedRef.current) {
      setLocalVolume(volume);
    }
  }, [volume, isDragging]);

  // Debounced save
  const debouncedSave = useCallback((newVolume: number) => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => onVolumeChange(newVolume), 100);
  }, [onVolumeChange]);

  // Cleanup
  useEffect(() => {
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
      if (commitTimeoutRef.current) clearTimeout(commitTimeoutRef.current);
    };
  }, []);

  // Register with meter coordinator — single RAF loop handles all rendering + DOM updates
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    // DPR setup
    const dpr = window.devicePixelRatio || 1;
    canvas.width = TOTAL_WIDTH * dpr;
    canvas.height = TOTAL_HEIGHT * dpr;
    ctx.scale(dpr, dpr);

    const stripId = trackId ?? '__master__';
    registerMeter(stripId, {
      canvas,
      ctx,
      trackId: isMaster ? null : (trackId ?? null),
      getConfig: () => configRef.current,
      peakDbEl: peakDbRef.current,
      clipEl: clipIndicatorRef.current,
    });

    return () => unregisterMeter(stripId);
  }, [trackId, isMaster]);

  // Volume from Y position
  const handleVolumeFromY = useCallback((clientY: number, fine: boolean = false) => {
    const container = containerRef.current;
    if (!container) return;

    const rect = container.getBoundingClientRect();
    const y = clientY - rect.top;
    const meterY = y - PADDING_Y;
    let newVolume = Math.max(0, Math.min(1, 1 - meterY / METER_HEIGHT));

    if (fine) {
      const db = linearToDb(newVolume);
      const roundedDb = Math.round(db * 10) / 10;
      newVolume = dbToLinear(roundedDb);
    }

    setLocalVolume(newVolume);
    debouncedSave(newVolume);
  }, [debouncedSave]);

  const commitVolume = useCallback((newVolume: number) => {
    recentlyCommittedRef.current = true;
    if (commitTimeoutRef.current) clearTimeout(commitTimeoutRef.current);
    commitTimeoutRef.current = setTimeout(() => {
      recentlyCommittedRef.current = false;
    }, 200);
    onVolumeChange(newVolume);
  }, [onVolumeChange]);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setIsDragging(true);
    handleVolumeFromY(e.clientY, e.ctrlKey || e.metaKey);
  }, [handleVolumeFromY]);

  useEffect(() => {
    if (!isDragging) return;

    let rafPending = false;
    let lastEvent: MouseEvent | null = null;

    const handleMouseMove = (e: MouseEvent) => {
      lastEvent = e;
      if (rafPending) return;
      rafPending = true;
      requestAnimationFrame(() => {
        if (lastEvent) {
          handleVolumeFromY(lastEvent.clientY, lastEvent.ctrlKey || lastEvent.metaKey);
        }
        rafPending = false;
      });
    };

    const handleMouseUp = () => {
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
        debounceRef.current = null;
      }
      commitVolume(localVolume);
      setIsDragging(false);
    };

    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);

    return () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isDragging, handleVolumeFromY, localVolume, commitVolume]);

  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    const currentDb = linearToDb(localVolume);
    const delta = e.deltaY > 0 ? -3 : 3;
    const newDb = Math.max(-60, Math.min(0, currentDb + delta));
    const newVolume = dbToLinear(newDb);
    setLocalVolume(newVolume);
    commitVolume(newVolume);
  }, [localVolume, commitVolume]);

  const handleDoubleClick = useCallback(() => {
    setLocalVolume(1.0);
    commitVolume(1.0);
  }, [commitVolume]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    const currentDb = linearToDb(localVolume);
    let newDb = currentDb;
    const step = e.ctrlKey || e.metaKey ? 0.1 : 1;

    switch (e.key) {
      case 'ArrowUp':
      case 'ArrowRight':
        e.preventDefault();
        newDb = Math.min(0, currentDb + step);
        break;
      case 'ArrowDown':
      case 'ArrowLeft':
        e.preventDefault();
        newDb = Math.max(-60, currentDb - step);
        break;
      case 'Home':
        e.preventDefault();
        newDb = 0;
        break;
      case 'End':
        e.preventDefault();
        newDb = -60;
        break;
      default:
        return;
    }

    const newVolume = dbToLinear(newDb);
    setLocalVolume(newVolume);
    commitVolume(newVolume);
  }, [localVolume, commitVolume]);

  const volumePercent = Math.round(localVolume * 100);

  const STRIP_WIDTH = METER_LAYOUT.stripWidth;

  return (
    <div className="flex flex-col gap-1.5" style={{ width: STRIP_WIDTH, contain: 'layout style' }}>
      {/* Control buttons */}
      {!isMaster ? (
        <div className="flex justify-center" style={{ marginLeft: METER_LAYOUT.labelOffset, width: BAR_WIDTH }}>
          <div className="flex gap-0.5 p-0.5 bg-[var(--bg-sunken)] border border-[var(--border-default)] rounded-md">
            <button
              type="button"
              className={`w-5 h-5 rounded flex items-center justify-center transition-colors text-[9px] font-bold ${
                muted
                  ? 'bg-red-500/20 text-red-400 border border-red-500/50'
                  : 'bg-[var(--bg-sunken)] text-[var(--text-muted)] hover:bg-[var(--bg-elevated)] hover:text-[var(--text-secondary)]'
              }`}
              onClick={() => onMuteToggle(!muted)}
              title={muted ? t('audio.unmute', { defaultValue: 'Unmute' }) : t('audio.mute', { defaultValue: 'Mute' })}
            >
              M
            </button>
            <button
              type="button"
              className={`w-5 h-5 rounded flex items-center justify-center transition-colors text-[9px] font-bold ${
                solo
                  ? 'bg-yellow-500/20 text-yellow-400 border border-yellow-500/50'
                  : 'bg-[var(--bg-sunken)] text-[var(--text-muted)] hover:bg-[var(--bg-elevated)] hover:text-[var(--text-secondary)]'
              }`}
              onClick={() => onSoloToggle(!solo)}
              title={solo ? t('audio.unsolo', { defaultValue: 'Unsolo' }) : t('audio.solo', { defaultValue: 'Solo' })}
            >
              S
            </button>
            {monitoringType && monitoringType !== 'none' && (
              <div
                className="w-5 h-5 rounded flex items-center justify-center bg-green-500/20 text-green-400 border border-green-500/50"
                title={monitoringType === 'monitorOnly' ? t('audio.monitorOnly', { defaultValue: 'Monitor Only' }) : t('audio.monitorAndOutput', { defaultValue: 'Monitor & Output' })}
              >
                <Headphones className="w-3 h-3" />
              </div>
            )}
            {trackId && onFiltersChange && (
              <AudioFilterButton
                trackId={trackId}
                trackName={label}
                filters={filters}
                onFiltersChange={onFiltersChange}
                availableSources={availableSources}
                compact
              />
            )}
          </div>
        </div>
      ) : (
        <div className="flex justify-center" style={{ marginLeft: METER_LAYOUT.labelOffset, width: BAR_WIDTH }}>
          <div className="flex gap-0.5 p-0.5 bg-[var(--bg-sunken)] border border-[var(--border-default)] rounded-md">
            <button
              type="button"
              className={`w-5 h-5 rounded flex items-center justify-center transition-colors text-[9px] font-bold ${
                muted
                  ? 'bg-red-500/20 text-red-400 border border-red-500/50'
                  : 'bg-[var(--bg-sunken)] text-[var(--text-muted)] hover:bg-[var(--bg-elevated)] hover:text-[var(--text-secondary)]'
              }`}
              onClick={() => onMuteToggle(!muted)}
              title={muted ? t('audio.unmute', { defaultValue: 'Unmute' }) : t('audio.mute', { defaultValue: 'Mute' })}
            >
              M
            </button>
          </div>
        </div>
      )}

      {/* Meter/fader */}
      <div
        ref={containerRef}
        className={`relative cursor-ns-resize select-none focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:ring-offset-1 rounded ${
          captureError ? 'opacity-60' : ''
        }`}
        style={{ width: TOTAL_WIDTH, height: TOTAL_HEIGHT, marginLeft: 8, contain: 'strict' }}
        tabIndex={0}
        role="slider"
        aria-label={`${label} volume`}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={volumePercent}
        aria-valuetext={`${formatDb(linearToDb(localVolume))} dB`}
        onMouseDown={handleMouseDown}
        onWheel={handleWheel}
        onDoubleClick={handleDoubleClick}
        onKeyDown={handleKeyDown}
        title={`${volumePercent}% (${formatDb(linearToDb(localVolume))} dB)`}
      >
        {/* Clip indicator — className managed by RAF loop via ref (no React re-render) */}
        <div
          ref={clipIndicatorRef}
          className="absolute rounded-t bg-transparent"
          style={{ left: LABEL_WIDTH, top: PADDING_Y - 4, width: BAR_WIDTH, height: 4 }}
        />

        {/* Canvas - rendered by meter coordinator (single RAF loop) */}
        <canvas
          ref={canvasRef}
          width={TOTAL_WIDTH}
          height={TOTAL_HEIGHT}
          style={{ width: TOTAL_WIDTH, height: TOTAL_HEIGHT, willChange: 'transform' }}
        />

        {/* No signal overlay */}
        {captureError && (
          <div
            className="absolute inset-0 flex items-center justify-center"
            style={{ left: LABEL_WIDTH, width: BAR_WIDTH, top: PADDING_Y, height: METER_HEIGHT }}
          >
            <div className="bg-black/30 rounded px-1 py-0.5">
              <span className="text-[8px] text-amber-400 font-medium">NO SIGNAL</span>
            </div>
          </div>
        )}
      </div>

      {/* Labels */}
      <div className="flex flex-col items-center gap-1" style={{ marginLeft: METER_LAYOUT.labelOffset, width: BAR_WIDTH }}>
        <div className="flex items-center gap-1.5 text-[10px] tabular-nums font-medium">
          <span ref={peakDbRef} className="px-1 py-0.5 rounded text-[var(--text-muted)]">-∞</span>
          <span className="text-[var(--text-muted)]">{volumePercent}%</span>
        </div>
        <span
          className={`text-[10px] text-center truncate ${
            isMaster ? 'font-semibold text-[var(--text-primary)]' : 'text-[var(--text-secondary)]'
          }`}
          style={{ maxWidth: METER_LAYOUT.maxControlWidth }}
          title={label}
        >
          {label}
        </span>
        <span
          className={`text-[9px] text-center truncate h-[14px] ${
            captureError ? 'text-amber-500' : 'text-transparent'
          }`}
          style={{ maxWidth: METER_LAYOUT.maxControlWidth }}
          title={captureError || undefined}
        >
          {captureError ? `⚠ ${t('audio.captureError', { defaultValue: 'No signal' })}` : '\u00A0'}
        </span>
        {/* Balance indicator */}
        {!isMaster && balance !== undefined && balance !== 0 && (
          <div className="flex items-center gap-1 text-[9px] text-[var(--text-muted)]" title={`Balance: ${balance > 0 ? `R ${Math.round(balance * 100)}%` : `L ${Math.round(Math.abs(balance) * 100)}%`}`}>
            <span>L</span>
            <div className="relative w-8 h-1 bg-[var(--bg-sunken)] rounded-full">
              <div
                className="absolute top-0 h-full bg-[var(--primary)] rounded-full"
                style={{
                  left: balance < 0 ? `${50 + balance * 50}%` : '50%',
                  width: `${Math.abs(balance) * 50}%`,
                }}
              />
              <div className="absolute top-0 left-1/2 w-px h-full bg-[var(--text-muted)] opacity-50" />
            </div>
            <span>R</span>
          </div>
        )}
      </div>
    </div>
  );
});
