/**
 * Unified Channel Strip
 * Combined VU meter + volume control in a single vertical bar
 * The VU meter IS the volume control - drag the arrow to adjust
 *
 * PERFORMANCE OPTIMIZATION:
 * This component uses RAF-based canvas rendering that reads audio levels
 * directly from a pure JS store (audioLevelStore), bypassing React's
 * render cycle. This eliminates ~30 re-renders per second.
 *
 * The canvas redraws only when:
 * 1. Audio level data version changes (dirty check)
 * 2. User is dragging the volume control
 */
import React, { useRef, useEffect, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { AudioFilterButton } from './AudioFilterButton';
import type { AudioFilter, Source } from '@/types/source';
import { linearToDb, dbToLinear } from '@/hooks/useAudioLevels';
import {
  getTrackLevel,
  getMasterLevel,
  getPeakHold,
  getMasterPeakHold,
  getVersion,
} from '@/lib/audio/audioLevelStore';
import {
  drawMeter,
  LABEL_WIDTH,
  BAR_WIDTH,
  ARROW_WIDTH,
  PADDING_Y,
  METER_HEIGHT,
  TOTAL_HEIGHT,
  TOTAL_WIDTH,
} from '@/lib/audio/meterRenderer';

// NOTE: Drawing constants and functions moved to meterRenderer.ts
// The component now uses RAF-based rendering that reads from audioLevelStore

export interface UnifiedChannelStripProps {
  /** Track ID for reading from audio store (undefined for master) */
  trackId?: string;
  label: string;
  // NOTE: Level props removed - component reads from audioLevelStore directly
  // This eliminates ~30 React re-renders per second
  // Controls
  volume: number;             // 0-1
  muted: boolean;
  solo: boolean;
  // Filters (for threshold display)
  filters?: AudioFilter[];
  // Is this the master channel?
  isMaster?: boolean;
  // Available sources for sidechain
  availableSources?: Source[];
  // Capture error message (if capture failed for this track)
  captureError?: string;
  // Callbacks
  onVolumeChange: (volume: number) => void;
  onMuteToggle: (muted: boolean) => void;
  onSoloToggle: (solo: boolean) => void;
  onFiltersChange?: (filters: AudioFilter[]) => void;
}

export const UnifiedChannelStrip = React.memo(function UnifiedChannelStrip({
  trackId,
  label,
  // NOTE: Level props removed - reads from audioLevelStore in RAF loop
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

  // Local volume state for responsive UI during drag
  const [localVolume, setLocalVolume] = useState(volume);
  const [isDragging, setIsDragging] = useState(false);
  // NOTE: peakHold state removed - managed by audioLevelStore
  const [showClip, setShowClip] = useState(false);

  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // NOTE: peakHoldTimeoutRef removed - peak hold managed by audioLevelStore
  const clipTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const animationFrameRef = useRef<number | null>(null);

  // Track recently committed volume to prevent sync race condition
  const recentlyCommittedRef = useRef(false);
  const commitTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Track last store version for dirty checking
  const lastVersionRef = useRef(-1);

  // NOTE: Dimension constants now imported from meterRenderer.ts

  // Sync local volume when prop changes (from server)
  // Skip sync if we recently committed a value (prevents race condition with stale props)
  useEffect(() => {
    if (!isDragging && !recentlyCommittedRef.current) {
      setLocalVolume(volume);
    }
  }, [volume, isDragging]);

  // NOTE: Peak hold effect removed - now managed by audioLevelStore

  // Handle clipping indicator - reads from store in RAF, triggers React state for CSS animation
  // This is the only React state update from audio data (once per clip event, not per frame)
  const lastClipRef = useRef(false);
  useEffect(() => {
    // Check clipping state periodically (much less frequent than RAF)
    const checkClipping = () => {
      const level = isMaster ? getMasterLevel() : (trackId ? getTrackLevel(trackId) : getMasterLevel());
      if (level.clipping && !lastClipRef.current) {
        lastClipRef.current = true;
        setShowClip(true);

        if (clipTimeoutRef.current) {
          clearTimeout(clipTimeoutRef.current);
        }
        clipTimeoutRef.current = setTimeout(() => {
          setShowClip(false);
          lastClipRef.current = false;
        }, 1000);
      }
    };

    // Check every 100ms instead of every frame
    const interval = setInterval(checkClipping, 100);
    return () => {
      clearInterval(interval);
      if (clipTimeoutRef.current) {
        clearTimeout(clipTimeoutRef.current);
      }
    };
  }, [trackId, isMaster]);

  // Debounced save to server
  const debouncedSave = useCallback((newVolume: number) => {
    if (debounceRef.current) {
      clearTimeout(debounceRef.current);
    }
    debounceRef.current = setTimeout(() => {
      onVolumeChange(newVolume);
    }, 100);
  }, [onVolumeChange]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
      if (clipTimeoutRef.current) clearTimeout(clipTimeoutRef.current);
      if (commitTimeoutRef.current) clearTimeout(commitTimeoutRef.current);
      if (animationFrameRef.current) cancelAnimationFrame(animationFrameRef.current);
    };
  }, []);

  // Get filters with thresholds for display
  const thresholdFilters = filters.filter(f =>
    f.enabled && ['noiseGate', 'compressor', 'expander'].includes(f.type)
  );

  // RAF-based canvas rendering
  // Reads audio levels directly from pure JS store, bypassing React
  // Only redraws when store version changes (dirty check) or during drag
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const context = canvas.getContext('2d', { alpha: false });
    if (!context) return;

    // Capture context in a const that TypeScript knows is non-null
    const ctx = context;

    // One-time setup: set canvas size with device pixel ratio
    const dpr = window.devicePixelRatio || 1;
    canvas.width = TOTAL_WIDTH * dpr;
    canvas.height = TOTAL_HEIGHT * dpr;
    ctx.scale(dpr, dpr);

    // Helper to get current render options
    const getRenderOptions = () => {
      const level = isMaster
        ? getMasterLevel()
        : (trackId ? getTrackLevel(trackId) : getMasterLevel());
      const peakHold = isMaster
        ? getMasterPeakHold()
        : (trackId ? getPeakHold(trackId) : getMasterPeakHold());
      return { level, peakHold, volume: localVolume, muted, isDragging, thresholdFilters };
    };

    // Draw immediately on mount (prevents flash)
    drawMeter(ctx, getRenderOptions());
    lastVersionRef.current = getVersion();

    let rafId: number;

    function render() {
      // Dirty check: only redraw if data changed or dragging
      const version = getVersion();
      if (version !== lastVersionRef.current || isDragging) {
        lastVersionRef.current = version;
        drawMeter(ctx, getRenderOptions());
      }

      rafId = requestAnimationFrame(render);
    }

    // Start RAF loop
    rafId = requestAnimationFrame(render);

    return () => {
      cancelAnimationFrame(rafId);
    };
  }, [trackId, isMaster, muted, localVolume, isDragging, thresholdFilters]);

  // Handle volume change from Y position
  const handleVolumeFromY = useCallback((clientY: number, fine: boolean = false) => {
    const container = containerRef.current;
    if (!container) return;

    const rect = container.getBoundingClientRect();
    const y = clientY - rect.top;
    // Account for padding: meter goes from PADDING_Y to PADDING_Y + METER_HEIGHT
    const meterY = y - PADDING_Y;
    // Invert: top = max (1.0), bottom = min (0)
    let newVolume = Math.max(0, Math.min(1, 1 - meterY / METER_HEIGHT));

    // Fine adjustment with Ctrl/Cmd key (0.1dB steps)
    if (fine) {
      const db = linearToDb(newVolume);
      const roundedDb = Math.round(db * 10) / 10; // Round to 0.1dB
      newVolume = dbToLinear(roundedDb);
    }

    setLocalVolume(newVolume);
    debouncedSave(newVolume);
  }, [debouncedSave]);

  // Commit volume change with race condition protection
  // Must be defined before useEffect that uses it
  const commitVolume = useCallback((newVolume: number) => {
    recentlyCommittedRef.current = true;
    if (commitTimeoutRef.current) {
      clearTimeout(commitTimeoutRef.current);
    }
    commitTimeoutRef.current = setTimeout(() => {
      recentlyCommittedRef.current = false;
    }, 200);
    onVolumeChange(newVolume);
  }, [onVolumeChange]);

  // Mouse handlers
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setIsDragging(true);
    handleVolumeFromY(e.clientY, e.ctrlKey || e.metaKey);
  }, [handleVolumeFromY]);

  useEffect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      handleVolumeFromY(e.clientY, e.ctrlKey || e.metaKey);
    };

    const handleMouseUp = () => {
      // Cancel any pending debounce
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
        debounceRef.current = null;
      }

      // Commit the final volume and release drag lock
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

  // Scroll wheel handler (±3dB per tick)
  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    const currentDb = linearToDb(localVolume);
    const delta = e.deltaY > 0 ? -3 : 3; // Scroll down = quieter
    const newDb = Math.max(-60, Math.min(0, currentDb + delta));
    const newVolume = dbToLinear(newDb);
    setLocalVolume(newVolume);
    commitVolume(newVolume);
  }, [localVolume, commitVolume]);

  // Double-click to reset to unity (0dB = volume 1.0)
  const handleDoubleClick = useCallback(() => {
    setLocalVolume(1.0);
    commitVolume(1.0);
  }, [commitVolume]);

  // Keyboard handler for arrow keys (accessibility)
  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    const currentDb = linearToDb(localVolume);
    let newDb = currentDb;
    const step = e.ctrlKey || e.metaKey ? 0.1 : 1; // Fine: 0.1dB, Normal: 1dB

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
        newDb = 0; // Max volume
        break;
      case 'End':
        e.preventDefault();
        newDb = -60; // Min volume
        break;
      default:
        return;
    }

    const newVolume = dbToLinear(newDb);
    setLocalVolume(newVolume);
    commitVolume(newVolume);
  }, [localVolume, commitVolume]);

  // Calculate display values
  const volumePercent = Math.round(localVolume * 100);

  // Ref for peak dB display - updated directly without React re-renders
  const peakDbRef = useRef<HTMLSpanElement>(null);

  // Update peak dB display directly via DOM (bypasses React render cycle)
  useEffect(() => {
    const updatePeakDb = () => {
      const span = peakDbRef.current;
      if (!span) return;

      const level = isMaster
        ? getMasterLevel()
        : (trackId ? getTrackLevel(trackId) : getMasterLevel());
      const db = level.peakDb;

      // Format dB
      const text = (db <= -60 || !isFinite(db)) ? '-∞' : db.toFixed(1);
      span.textContent = text;

      // Update color classes based on level
      span.className = `px-1 py-0.5 rounded ${
        db > -3
          ? 'text-red-400 bg-red-500/10'
          : db > -10
          ? 'text-yellow-400 bg-yellow-500/10'
          : 'text-[var(--text-muted)]'
      }`;
    };

    // Update at 10Hz - direct DOM update, no React re-renders
    const interval = setInterval(updatePeakDb, 100);
    updatePeakDb(); // Initial update

    return () => clearInterval(interval);
  }, [trackId, isMaster]);

  // Format dB display (used for aria-valuetext)
  const formatDb = (db: number): string => {
    if (db <= -60 || !isFinite(db)) return '-∞';
    return db.toFixed(1);
  };

  // Wider strip for spacing, but buttons centered over bar
  const STRIP_WIDTH = TOTAL_WIDTH + 16; // Add padding for spacing between strips

  return (
    <div className="flex flex-col gap-1.5" style={{ width: STRIP_WIDTH }}>
      {/* Top section: Control buttons - centered over the bar */}
      {!isMaster ? (
        <div
          className="flex justify-center"
          style={{ marginLeft: LABEL_WIDTH + 8, width: BAR_WIDTH }}
        >
          <div className="flex gap-0.5 p-0.5 bg-[var(--bg-sunken)] border border-[var(--border-default)] rounded-md">
          {/* Mute button */}
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

          {/* Solo button */}
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

          {/* Filter button */}
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
        /* Master channel header - same style as inputs but with just mute button */
        <div
          className="flex justify-center"
          style={{ marginLeft: LABEL_WIDTH + 8, width: BAR_WIDTH }}
        >
          <div className="flex gap-0.5 p-0.5 bg-[var(--bg-sunken)] border border-[var(--border-default)] rounded-md">
            {/* Mute button */}
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

      {/* Main section: Unified meter/fader bar */}
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
        title={`${volumePercent}% (${formatDb(linearToDb(localVolume))} dB) - Drag to adjust, scroll for ±3dB, double-click for 0dB, arrow keys for ±1dB`}
      >
        {/* Clip indicator - positioned over meter bar */}
        <div
          className={`absolute rounded-t transition-colors ${
            showClip
              ? 'bg-red-500 animate-pulse'
              : 'bg-transparent'
          }`}
          style={{ left: LABEL_WIDTH, top: PADDING_Y - 4, width: BAR_WIDTH, height: 4 }}
        />

        {/* Canvas meter with labels */}
        <canvas
          ref={canvasRef}
          style={{
            width: TOTAL_WIDTH,
            height: TOTAL_HEIGHT,
          }}
        />

        {/* No signal overlay - shown when capture error exists */}
        {captureError && (
          <div
            className="absolute inset-0 flex items-center justify-center"
            style={{ left: LABEL_WIDTH, width: BAR_WIDTH, top: PADDING_Y, height: METER_HEIGHT }}
          >
            <div className="bg-black/30 rounded px-1 py-0.5">
              <span className="text-[8px] text-amber-400 font-medium">
                NO SIGNAL
              </span>
            </div>
          </div>
        )}
      </div>

      {/* Bottom section: centered over the bar */}
      <div
        className="flex flex-col items-center gap-1"
        style={{ marginLeft: LABEL_WIDTH + 8, width: BAR_WIDTH }}
      >
        {/* dB and volume display */}
        <div className="flex items-center gap-1.5 text-[10px] tabular-nums font-medium">
          <span
            ref={peakDbRef}
            className="px-1 py-0.5 rounded text-[var(--text-muted)]"
          >
            -∞
          </span>
          <span className="text-[var(--text-muted)]">
            {volumePercent}%
          </span>
        </div>

        {/* Track label */}
        <span
          className={`text-[10px] text-center truncate ${
            isMaster ? 'font-semibold text-[var(--text-primary)]' : 'text-[var(--text-secondary)]'
          }`}
          style={{ maxWidth: BAR_WIDTH + ARROW_WIDTH }}
          title={label}
        >
          {label}
        </span>

        {/* Capture status indicator - always reserve space for consistent height */}
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
