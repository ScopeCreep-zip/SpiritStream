/**
 * Unified Channel Strip
 * Combined VU meter + volume control in a single vertical bar
 * The VU meter IS the volume control - drag the arrow to adjust
 */
import React, { useRef, useEffect, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { AudioFilterButton } from './AudioFilterButton';
import type { AudioFilter, Source } from '@/types/source';
import { linearToDb, dbToLinear } from '@/hooks/useAudioLevels';

/**
 * Convert dB to visual position (percentage from bottom)
 * Linear dB scale: 0dB = 100%, -60dB = 0%
 * Matches plan spec: -20dB = 67%, -40dB = 33%, etc.
 */
function dbToPosition(db: number): number {
  const clampedDb = Math.max(-60, Math.min(0, db));
  return ((clampedDb + 60) / 60) * 100;
}

/**
 * Convert linear amplitude (0-1) to visual position
 * First converts to dB, then to position
 */
function linearToPosition(linear: number): number {
  if (linear <= 0) return 0;
  const db = 20 * Math.log10(linear);
  return dbToPosition(db);
}

// Threshold marker colors
const THRESHOLD_COLORS: Record<string, string> = {
  noiseGate: '#a855f7',    // Purple
  compressor: '#3b82f6',   // Blue
  expander: '#f59e0b',     // Amber
};

export interface UnifiedChannelStripProps {
  trackId?: string;
  label: string;
  // Levels (real-time from WebSocket)
  rmsLevel: number;           // 0-1
  peakLevel: number;          // 0-1
  peakDb: number;             // dB value for display
  isClipping: boolean;
  // Stereo support (optional) - if provided, shows separate L/R levels
  // If not provided, duplicates mono data for both channels
  leftRms?: number;           // Left channel RMS
  leftPeak?: number;          // Left channel peak
  rightRms?: number;          // Right channel RMS
  rightPeak?: number;         // Right channel peak
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
  // Callbacks
  onVolumeChange: (volume: number) => void;
  onMuteToggle: (muted: boolean) => void;
  onSoloToggle: (solo: boolean) => void;
  onFiltersChange?: (filters: AudioFilter[]) => void;
}

export const UnifiedChannelStrip = React.memo(function UnifiedChannelStrip({
  trackId,
  label,
  rmsLevel,
  peakLevel,
  peakDb,
  isClipping,
  leftRms,
  leftPeak,
  rightRms,
  rightPeak,
  volume,
  muted,
  solo,
  filters = [],
  isMaster = false,
  availableSources = [],
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
  const [peakHold, setPeakHold] = useState(0);
  const [showClip, setShowClip] = useState(false);

  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const peakHoldTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const clipTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const animationFrameRef = useRef<number | null>(null);

  // Track recently committed volume to prevent sync race condition
  // When we commit a volume change, we set this to true and clear it after a delay
  // This prevents the sync effect from reverting to the stale prop value
  const recentlyCommittedRef = useRef(false);
  const commitTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Bar dimensions - consistent for all channels with stereo L/R
  const LABEL_WIDTH = 22;  // Space for dB labels on left
  const BAR_WIDTH = 28;  // Meter bar width (same for all channels)
  const ARROW_WIDTH = 8;   // Arrow width
  const ARROW_HEIGHT = 12; // Arrow height
  const PADDING_Y = 8;  // Vertical padding for labels at top/bottom
  const METER_HEIGHT = 160;  // Inner meter height
  const TOTAL_HEIGHT = METER_HEIGHT + PADDING_Y * 2;  // Total canvas height
  const TOTAL_WIDTH = LABEL_WIDTH + BAR_WIDTH + ARROW_WIDTH;

  // Sync local volume when prop changes (from server)
  // Skip sync if we recently committed a value (prevents race condition with stale props)
  useEffect(() => {
    if (!isDragging && !recentlyCommittedRef.current) {
      setLocalVolume(volume);
    }
  }, [volume, isDragging]);

  // Update peak hold with 20s hold then decay (OBS parity - 20 seconds instead of 1.5s)
  useEffect(() => {
    if (peakLevel > peakHold) {
      setPeakHold(peakLevel);

      if (peakHoldTimeoutRef.current) {
        clearTimeout(peakHoldTimeoutRef.current);
      }

      peakHoldTimeoutRef.current = setTimeout(() => {
        const decayInterval = setInterval(() => {
          setPeakHold((prev) => {
            const newVal = prev - 0.02;
            if (newVal <= 0.01) {
              clearInterval(decayInterval);
              return 0;
            }
            return newVal;
          });
        }, 50);
      }, 20000); // OBS uses 20 second peak hold
    }

    return () => {
      if (peakHoldTimeoutRef.current) {
        clearTimeout(peakHoldTimeoutRef.current);
      }
    };
  }, [peakLevel, peakHold]);

  // Handle clipping indicator (1s flash duration)
  useEffect(() => {
    if (isClipping) {
      setShowClip(true);

      if (clipTimeoutRef.current) {
        clearTimeout(clipTimeoutRef.current);
      }

      clipTimeoutRef.current = setTimeout(() => {
        setShowClip(false);
      }, 1000);
    }

    return () => {
      if (clipTimeoutRef.current) {
        clearTimeout(clipTimeoutRef.current);
      }
    };
  }, [isClipping]);

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
      if (peakHoldTimeoutRef.current) clearTimeout(peakHoldTimeoutRef.current);
      if (clipTimeoutRef.current) clearTimeout(clipTimeoutRef.current);
      if (commitTimeoutRef.current) clearTimeout(commitTimeoutRef.current);
      if (animationFrameRef.current) cancelAnimationFrame(animationFrameRef.current);
    };
  }, []);

  // Get filters with thresholds for display
  const thresholdFilters = filters.filter(f =>
    f.enabled && ['noiseGate', 'compressor', 'expander'].includes(f.type)
  );

  // Draw the unified meter/fader on canvas
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;

    // Set canvas size accounting for device pixel ratio
    canvas.width = TOTAL_WIDTH * dpr;
    canvas.height = TOTAL_HEIGHT * dpr;
    ctx.scale(dpr, dpr);

    // Clear canvas
    ctx.clearRect(0, 0, TOTAL_WIDTH, TOTAL_HEIGHT);

    // Meter bar starts after the label area, with vertical padding
    const meterX = LABEL_WIDTH;
    const meterTop = PADDING_Y;
    const meterBottom = PADDING_Y + METER_HEIGHT;

    // 1. Background fill (dark) - only for meter area
    ctx.fillStyle = 'rgba(0, 0, 0, 0.5)';
    ctx.beginPath();
    ctx.roundRect(meterX, meterTop, BAR_WIDTH, METER_HEIGHT, 4);
    ctx.fill();

    // Helper to convert dB position (0-100%) to Y coordinate within meter area
    const dbToY = (position: number) => meterBottom - (position / 100) * METER_HEIGHT;

    // 2. VU level fill (gradient) - draw first so dB markers appear on top
    // OBS Parity: Show PEAK as the main fill, RMS as a bright line on top
    // Always show stereo (L/R split) - use same data for both if stereo data not provided

    // Create gradient matching plan's dB color ranges (muted colors):
    // Green: -60dB to -20dB (0% to 67%)
    // Yellow: -20dB to -6dB (67% to 90%)
    // Orange: -6dB to -3dB (90% to 95%)
    // Red: -3dB to 0dB (95% to 100%)
    const gradient = ctx.createLinearGradient(0, meterBottom, 0, meterTop);
    gradient.addColorStop(0, '#166534');      // Muted green at -60dB
    gradient.addColorStop(0.67, '#15803d');   // Green up to -20dB
    gradient.addColorStop(0.67, '#a16207');   // Muted yellow starts at -20dB
    gradient.addColorStop(0.90, '#ca8a04');   // Yellow up to -6dB
    gradient.addColorStop(0.90, '#c2410c');   // Muted orange starts at -6dB
    gradient.addColorStop(0.95, '#ea580c');   // Orange up to -3dB
    gradient.addColorStop(0.95, '#b91c1c');   // Muted red starts at -3dB
    gradient.addColorStop(1, '#dc2626');      // Red at 0dB

    // Always show stereo mode - use provided L/R data or duplicate mono data
    const channelWidth = (BAR_WIDTH - 4) / 2 - 1; // Leave 2px gap in middle
    const effectiveLeftPeak = muted ? 0 : (leftPeak ?? peakLevel);
    const effectiveRightPeak = muted ? 0 : (rightPeak ?? peakLevel);
    const effectiveLeftRms = muted ? 0 : (leftRms ?? rmsLevel);
    const effectiveRightRms = muted ? 0 : (rightRms ?? rmsLevel);

    // Left channel
    const leftPeakPosition = linearToPosition(effectiveLeftPeak);
    const leftPeakHeight = (leftPeakPosition / 100) * METER_HEIGHT;
    if (leftPeakHeight > 0) {
      ctx.fillStyle = gradient;
      ctx.beginPath();
      ctx.roundRect(meterX + 2, meterBottom - leftPeakHeight, channelWidth, leftPeakHeight, [0, 0, 2, 2]);
      ctx.fill();
    }

    // Right channel
    const rightX = meterX + 2 + channelWidth + 2; // 2px gap
    const rightPeakPosition = linearToPosition(effectiveRightPeak);
    const rightPeakHeight = (rightPeakPosition / 100) * METER_HEIGHT;
    if (rightPeakHeight > 0) {
      ctx.fillStyle = gradient;
      ctx.beginPath();
      ctx.roundRect(rightX, meterBottom - rightPeakHeight, channelWidth, rightPeakHeight, [0, 0, 2, 2]);
      ctx.fill();
    }

    // Draw RMS lines ON TOP of peak fill - bright white line with dark outline for visibility
    const leftRmsPosition = linearToPosition(effectiveLeftRms);
    if (leftRmsPosition > 0 && effectiveLeftPeak > 0) {
      const leftRmsY = dbToY(leftRmsPosition);
      // Dark outline
      ctx.fillStyle = '#000000';
      ctx.fillRect(meterX + 2, leftRmsY - 2, channelWidth, 4);
      // Bright inner line
      ctx.fillStyle = '#ffffff';
      ctx.fillRect(meterX + 2, leftRmsY - 1, channelWidth, 2);
    }

    const rightRmsPosition = linearToPosition(effectiveRightRms);
    if (rightRmsPosition > 0 && effectiveRightPeak > 0) {
      const rightRmsY = dbToY(rightRmsPosition);
      // Dark outline
      ctx.fillStyle = '#000000';
      ctx.fillRect(rightX, rightRmsY - 2, channelWidth, 4);
      // Bright inner line
      ctx.fillStyle = '#ffffff';
      ctx.fillRect(rightX, rightRmsY - 1, channelWidth, 2);
    }

    // Draw L/R labels at top of each channel
    ctx.font = '6px system-ui, sans-serif';
    ctx.textAlign = 'center';
    ctx.fillStyle = 'rgba(255, 255, 255, 0.7)';
    ctx.fillText('L', meterX + 2 + channelWidth / 2, meterTop + 8);
    ctx.fillText('R', rightX + channelWidth / 2, meterTop + 8);

    // Draw stereo separator line (bright line between L and R)
    const separatorX = meterX + 2 + channelWidth + 1; // Center of the 2px gap
    ctx.fillStyle = 'rgba(255, 255, 255, 0.5)';
    ctx.fillRect(separatorX - 0.5, meterTop, 1, METER_HEIGHT);

    // 3. dB scale markers - drawn ON TOP of VU fill so they're visible
    ctx.font = '8px system-ui, sans-serif';
    ctx.textAlign = 'right';
    ctx.textBaseline = 'middle';

    const labelMarkers = [0, -5, -10, -15, -20, -25, -30, -35, -40, -45, -50, -55, -60];
    labelMarkers.forEach(db => {
      const position = dbToPosition(db);
      const y = dbToY(position);

      // Soft tick mark across meter
      ctx.fillStyle = 'rgba(255, 255, 255, 0.2)';
      ctx.fillRect(meterX + 1, y - 0.5, BAR_WIDTH - 2, 1);

      // Tick mark extending left from meter
      ctx.fillStyle = 'rgba(255, 255, 255, 0.4)';
      ctx.fillRect(meterX - 4, y - 0.5, 4, 1);

      // dB label
      ctx.fillStyle = 'rgba(255, 255, 255, 0.6)';
      const label = db === 0 ? '0' : String(db);
      ctx.fillText(label, meterX - 5, y);
    });

    // Reference levels: -20 dBFS and -9 dBFS (brighter markers)
    const referenceMarkers = [
      { db: -20, color: 'rgba(34, 197, 94, 0.9)' },   // Green for -20dB
      { db: -9, color: 'rgba(234, 179, 8, 0.9)' },    // Yellow for -9dB
    ];
    referenceMarkers.forEach(({ db, color }) => {
      const position = dbToPosition(db);
      const y = dbToY(position);
      ctx.fillStyle = color;
      ctx.fillRect(meterX + 1, y - 1, BAR_WIDTH - 2, 2);
    });

    // Threshold markers for active filters (dashed lines)
    thresholdFilters.forEach(filter => {
      const threshold = (filter as { threshold?: number }).threshold ?? -40;
      const position = dbToPosition(threshold);
      const y = dbToY(position);
      const color = THRESHOLD_COLORS[filter.type] || '#888';

      ctx.strokeStyle = color;
      ctx.lineWidth = 2;
      ctx.setLineDash([4, 3]);
      ctx.beginPath();
      ctx.moveTo(meterX, y);
      ctx.lineTo(meterX + BAR_WIDTH, y);
      ctx.stroke();
      ctx.setLineDash([]);
    });

    // 5. Peak hold line (bright) - spans both channels
    const effectivePeak = muted ? 0 : peakHold;
    if (effectivePeak > 0.01) {
      const peakPosition = linearToPosition(effectivePeak);
      const peakY = dbToY(peakPosition);
      // Color based on dB level
      const peakDbVal = effectivePeak > 0 ? 20 * Math.log10(effectivePeak) : -60;
      let peakColor = '#22c55e';  // Green (below -20dB)
      if (peakDbVal > -3) peakColor = '#ef4444';       // Red (above -3dB)
      else if (peakDbVal > -6) peakColor = '#f97316';  // Orange (-6 to -3dB)
      else if (peakDbVal > -20) peakColor = '#eab308'; // Yellow (-20 to -6dB)

      ctx.fillStyle = peakColor;
      ctx.fillRect(meterX + 2, peakY - 1.5, BAR_WIDTH - 4, 3);
    }

    // 6. Volume arrow (triangle marker on right side of meter, pointing left into it)
    // Per plan: filled when dragging, outline when idle
    const volumeY = meterBottom - localVolume * METER_HEIGHT;
    const arrowX = meterX + BAR_WIDTH; // Arrow starts at the right edge of the meter

    ctx.beginPath();
    ctx.moveTo(arrowX, volumeY);                                    // Tip (pointing left into meter)
    ctx.lineTo(arrowX + ARROW_WIDTH, volumeY - ARROW_HEIGHT / 2);   // Top right
    ctx.lineTo(arrowX + ARROW_WIDTH, volumeY + ARROW_HEIGHT / 2);   // Bottom right
    ctx.closePath();

    if (isDragging) {
      // Filled when dragging
      ctx.fillStyle = '#7C3AED';
      ctx.fill();
    } else {
      // Outline when idle
      ctx.strokeStyle = 'rgba(255, 255, 255, 0.9)';
      ctx.lineWidth = 2;
      ctx.stroke();
    }

  }, [rmsLevel, peakLevel, peakHold, localVolume, muted, thresholdFilters, isDragging, leftRms, leftPeak, rightRms, rightPeak, BAR_WIDTH, METER_HEIGHT, TOTAL_HEIGHT, TOTAL_WIDTH, LABEL_WIDTH, PADDING_Y]);

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

  // Format dB display
  const formatDb = (db: number): string => {
    if (db <= -60) return '-∞';
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
        className="relative cursor-ns-resize select-none focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:ring-offset-1 rounded"
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
      </div>

      {/* Bottom section: centered over the bar */}
      <div
        className="flex flex-col items-center gap-1"
        style={{ marginLeft: LABEL_WIDTH + 8, width: BAR_WIDTH }}
      >
        {/* dB and volume display */}
        <div className="flex items-center gap-1.5 text-[10px] tabular-nums font-medium">
          <span
            className={`px-1 py-0.5 rounded ${
              peakDb > -3
                ? 'text-red-400 bg-red-500/10'
                : peakDb > -10
                ? 'text-yellow-400 bg-yellow-500/10'
                : 'text-[var(--text-muted)]'
            }`}
          >
            {formatDb(peakDb)}
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
      </div>
    </div>
  );
});
