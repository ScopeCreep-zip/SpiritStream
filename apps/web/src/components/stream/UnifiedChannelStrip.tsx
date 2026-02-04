/**
 * Unified Channel Strip
 * Combined VU meter + volume control in a single vertical bar
 *
 * ARCHITECTURE (Comlink-inspired):
 * - registerMeterCanvas() returns a Promise that resolves when canvas is
 *   actually transferred to the worker (auto-queued if worker not ready)
 * - No fallback rendering needed - worker handles all canvas drawing
 * - Main thread only does lightweight DOM updates (peak dB, clipping)
 *
 * References:
 * - Comlink pattern: https://github.com/GoogleChromeLabs/comlink
 * - OffscreenCanvas: https://web.dev/articles/offscreen-canvas
 */
import React, { useRef, useEffect, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { AudioFilterButton } from './AudioFilterButton';
import type { AudioFilter, Source } from '@/types/source';
import { linearToDb, dbToLinear } from '@/hooks/useAudioLevels';
import {
  getTrackLevel,
  getMasterLevel,
} from '@/lib/audio/audioLevelStore';
import {
  LABEL_WIDTH,
  BAR_WIDTH,
  ARROW_WIDTH,
  PADDING_Y,
  METER_HEIGHT,
  TOTAL_HEIGHT,
  TOTAL_WIDTH,
} from '@/lib/audio/meterRenderer';
import {
  registerMeterCanvas,
  updateMeterConfig,
  isCanvasRegistered,
  getCanvasId,
} from '@/lib/audio/audioMeterWorkerBridge';

// Check if OffscreenCanvas is supported
const supportsOffscreenCanvas = typeof OffscreenCanvas !== 'undefined' &&
  typeof HTMLCanvasElement.prototype.transferControlToOffscreen === 'function';

export interface UnifiedChannelStripProps {
  trackId?: string;
  label: string;
  volume: number;
  muted: boolean;
  solo: boolean;
  filters?: AudioFilter[];
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
  const [showClip, setShowClip] = useState(false);
  const [isRegistered, setIsRegistered] = useState(false);

  const workerCanvasIdRef = useRef<string | null>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const clipTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const recentlyCommittedRef = useRef(false);
  const commitTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastClipRef = useRef(false);
  const peakDbRef = useRef<HTMLSpanElement>(null);

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
      if (clipTimeoutRef.current) clearTimeout(clipTimeoutRef.current);
      if (commitTimeoutRef.current) clearTimeout(commitTimeoutRef.current);
    };
  }, []);

  // Canvas registration - Promise-based, auto-queued
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !supportsOffscreenCanvas) return;

    // Already registered (React StrictMode)
    if (isCanvasRegistered(canvas)) {
      const existingId = getCanvasId(canvas);
      if (existingId) {
        workerCanvasIdRef.current = existingId;
        setIsRegistered(true);
        return;
      }
    }

    // Register canvas - Promise resolves when actually transferred to worker
    let cancelled = false;

    registerMeterCanvas(
      canvas,
      isMaster ? null : (trackId || null),
      { volume: localVolume, muted }
    ).then((canvasId) => {
      if (cancelled) return;
      workerCanvasIdRef.current = canvasId;
      setIsRegistered(true);
    }).catch((err) => {
      if (cancelled) return;
      console.error('[UnifiedChannelStrip] Canvas registration failed:', err);
    });

    return () => {
      cancelled = true;
    };
  }, [trackId, isMaster]);

  // Send config updates to worker (volume, muted, dragging state)
  useEffect(() => {
    if (isRegistered && workerCanvasIdRef.current) {
      updateMeterConfig(workerCanvasIdRef.current, { volume: localVolume, muted, isDragging });
    }
  }, [localVolume, muted, isDragging, isRegistered]);

  // Send filter threshold updates to worker
  useEffect(() => {
    if (isRegistered && workerCanvasIdRef.current && filters.length > 0) {
      // Extract threshold filters (noise gate, compressor, expander)
      const thresholdFilters = filters
        .filter(f => ['noiseGate', 'compressor', 'expander'].includes(f.type))
        .map(f => ({
          type: f.type,
          threshold: (f as { threshold?: number }).threshold,
          enabled: f.enabled,
        }));
      updateMeterConfig(workerCanvasIdRef.current, { thresholdFilters });
    }
  }, [filters, isRegistered]);

  // DOM updates: peak dB display and clipping (10Hz, lightweight)
  useEffect(() => {
    let rafId: number;
    let lastUpdateTime = 0;
    const UPDATE_INTERVAL = 100;

    const getLevel = () => isMaster
      ? getMasterLevel()
      : (trackId ? getTrackLevel(trackId) : getMasterLevel());

    const updateDom = () => {
      const level = getLevel();

      const span = peakDbRef.current;
      if (span) {
        const db = level.peakDb;
        const text = (db <= -60 || !isFinite(db)) ? '-∞' : db.toFixed(1);
        span.textContent = text;
        span.className = `px-1 py-0.5 rounded ${
          db > -3 ? 'text-red-400 bg-red-500/10'
            : db > -10 ? 'text-yellow-400 bg-yellow-500/10'
            : 'text-[var(--text-muted)]'
        }`;
      }

      if (level.clipping && !lastClipRef.current) {
        lastClipRef.current = true;
        setShowClip(true);
        if (clipTimeoutRef.current) clearTimeout(clipTimeoutRef.current);
        clipTimeoutRef.current = setTimeout(() => {
          setShowClip(false);
          lastClipRef.current = false;
        }, 1000);
      }
    };

    updateDom();

    const domUpdateLoop = (timestamp: number) => {
      if (timestamp - lastUpdateTime >= UPDATE_INTERVAL) {
        lastUpdateTime = timestamp;
        updateDom();
      }
      rafId = requestAnimationFrame(domUpdateLoop);
    };

    rafId = requestAnimationFrame(domUpdateLoop);

    return () => {
      cancelAnimationFrame(rafId);
      if (clipTimeoutRef.current) clearTimeout(clipTimeoutRef.current);
    };
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

  const formatDb = (db: number): string => {
    if (db <= -60 || !isFinite(db)) return '-∞';
    return db.toFixed(1);
  };

  const STRIP_WIDTH = TOTAL_WIDTH + 16;

  return (
    <div className="flex flex-col gap-1.5" style={{ width: STRIP_WIDTH }}>
      {/* Control buttons */}
      {!isMaster ? (
        <div className="flex justify-center" style={{ marginLeft: LABEL_WIDTH + 8, width: BAR_WIDTH }}>
          <div className="flex gap-0.5 p-0.5 bg-[var(--bg-sunken)] border border-[var(--border-default)] rounded-md">
            <button
              type="button"
              className={`w-5 h-5 rounded flex items-center justify-center transition-all text-[9px] font-bold ${
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
              className={`w-5 h-5 rounded flex items-center justify-center transition-all text-[9px] font-bold ${
                solo
                  ? 'bg-yellow-500/20 text-yellow-400 border border-yellow-500/50'
                  : 'bg-[var(--bg-sunken)] text-[var(--text-muted)] hover:bg-[var(--bg-elevated)] hover:text-[var(--text-secondary)]'
              }`}
              onClick={() => onSoloToggle(!solo)}
              title={solo ? t('audio.unsolo', { defaultValue: 'Unsolo' }) : t('audio.solo', { defaultValue: 'Solo' })}
            >
              S
            </button>
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
        <div className="flex justify-center" style={{ marginLeft: LABEL_WIDTH + 8, width: BAR_WIDTH }}>
          <div className="flex gap-0.5 p-0.5 bg-[var(--bg-sunken)] border border-[var(--border-default)] rounded-md">
            <button
              type="button"
              className={`w-5 h-5 rounded flex items-center justify-center transition-all text-[9px] font-bold ${
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
        className={`relative cursor-ns-resize select-none focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:ring-offset-1 rounded transition-opacity ${
          captureError ? 'opacity-60' : ''
        }`}
        style={{ width: TOTAL_WIDTH, height: TOTAL_HEIGHT, marginLeft: 8 }}
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
        {/* Clip indicator */}
        <div
          className={`absolute rounded-t transition-colors ${
            showClip ? 'bg-red-500 animate-pulse' : 'bg-transparent'
          }`}
          style={{ left: LABEL_WIDTH, top: PADDING_Y - 4, width: BAR_WIDTH, height: 4 }}
        />

        {/* Canvas - rendered by worker */}
        <canvas
          ref={canvasRef}
          style={{ width: TOTAL_WIDTH, height: TOTAL_HEIGHT }}
        />

        {/* Loading shimmer while worker initializes */}
        {!isRegistered && supportsOffscreenCanvas && (
          <div
            className="absolute bg-[var(--bg-sunken)] overflow-hidden"
            style={{ left: LABEL_WIDTH, top: PADDING_Y, width: BAR_WIDTH, height: METER_HEIGHT }}
          >
            <div className="absolute inset-0 bg-gradient-to-b from-transparent via-[var(--bg-elevated)]/30 to-transparent skeleton-shimmer" />
          </div>
        )}

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
      <div className="flex flex-col items-center gap-1" style={{ marginLeft: LABEL_WIDTH + 8, width: BAR_WIDTH }}>
        <div className="flex items-center gap-1.5 text-[10px] tabular-nums font-medium">
          <span ref={peakDbRef} className="px-1 py-0.5 rounded text-[var(--text-muted)]">-∞</span>
          <span className="text-[var(--text-muted)]">{volumePercent}%</span>
        </div>
        <span
          className={`text-[10px] text-center truncate ${
            isMaster ? 'font-semibold text-[var(--text-primary)]' : 'text-[var(--text-secondary)]'
          }`}
          style={{ maxWidth: BAR_WIDTH + ARROW_WIDTH }}
          title={label}
        >
          {label}
        </span>
        <span
          className={`text-[9px] text-center truncate h-[14px] ${
            captureError ? 'text-amber-500' : 'text-transparent'
          }`}
          style={{ maxWidth: BAR_WIDTH + ARROW_WIDTH }}
          title={captureError || undefined}
        >
          {captureError ? `⚠ ${t('audio.captureError', { defaultValue: 'No signal' })}` : '\u00A0'}
        </span>
      </div>
    </div>
  );
});
