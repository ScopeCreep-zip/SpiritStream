/**
 * Meter Coordinator
 *
 * Single RAF loop that renders all audio meters. Mirrors OBS's
 * VolumeMeterTimer singleton (one 34ms timer iterates all meters).
 *
 * Components register/unregister their canvas + DOM refs on mount/unmount.
 * The coordinator reads level data from audioLevelStore and calls
 * drawMeter() from meterRenderer for each registered meter.
 */

import { drawMeter } from './meterRenderer';
import {
  getTrackLevel,
  getMasterLevel,
  getPeakHold,
  getMasterPeakHold,
  getVersion,
} from './audioLevelStore';
import type { AudioFilter } from '@/types/source';

// ============================================================================
// Types
// ============================================================================

export interface MeterConfig {
  volume: number;
  muted: boolean;
  isDragging: boolean;
  thresholdFilters: AudioFilter[];
}

export interface MeterRegistration {
  canvas: HTMLCanvasElement;
  ctx: CanvasRenderingContext2D;
  /** null = master track */
  trackId: string | null;
  /** Returns fresh config each call — reads from component refs */
  getConfig: () => MeterConfig;
  /** Peak dB text element — updated at 10Hz */
  peakDbEl: HTMLSpanElement | null;
  /** Clipping indicator element */
  clipEl: HTMLDivElement | null;
}

// ============================================================================
// State
// ============================================================================

const meters = new Map<string, MeterRegistration>();
let rafId: number | null = null;
let paused = false;
let lastRenderTime = 0;
let lastDomTime = 0;
let lastVersion = -1;

// Clipping state per meter (avoid re-triggering animation)
const clipState = new Map<string, { active: boolean; timeout: ReturnType<typeof setTimeout> | null }>();

const RENDER_INTERVAL = 34; // ~29fps (OBS VolumeMeterTimer uses 34ms)
const DOM_INTERVAL = 100;   // 10Hz for text updates

// ============================================================================
// RAF Loop
// ============================================================================

function loop(timestamp: number): void {
  if (paused || meters.size === 0) {
    rafId = requestAnimationFrame(loop);
    return;
  }

  const shouldRenderCanvas = timestamp - lastRenderTime >= RENDER_INTERVAL;
  const currentVersion = getVersion();
  const shouldUpdateDom = timestamp - lastDomTime >= DOM_INTERVAL && currentVersion !== lastVersion;

  if (shouldRenderCanvas) {
    lastRenderTime = timestamp;

    for (const [, reg] of meters) {
      const config = reg.getConfig();
      const level = reg.trackId ? getTrackLevel(reg.trackId) : getMasterLevel();
      const peakHold = reg.trackId ? getPeakHold(reg.trackId) : getMasterPeakHold();

      drawMeter(reg.ctx, {
        level,
        peakHold,
        volume: config.volume,
        muted: config.muted,
        isDragging: config.isDragging,
        thresholdFilters: config.thresholdFilters,
      });
    }
  }

  if (shouldUpdateDom) {
    lastDomTime = timestamp;
    lastVersion = currentVersion;

    for (const [id, reg] of meters) {
      updateMeterDom(id, reg);
    }
  }

  rafId = requestAnimationFrame(loop);
}

function updateMeterDom(id: string, reg: MeterRegistration): void {
  const level = reg.trackId ? getTrackLevel(reg.trackId) : getMasterLevel();

  // Peak dB text
  if (reg.peakDbEl) {
    const db = level.peakDb;
    const text = (db <= -60 || !isFinite(db)) ? '-\u221E' : db.toFixed(1);
    reg.peakDbEl.textContent = text;
    reg.peakDbEl.className = `px-1 py-0.5 rounded ${
      db > -3 ? 'text-red-400 bg-red-500/10'
        : db > -10 ? 'text-yellow-400 bg-yellow-500/10'
        : 'text-[var(--text-muted)]'
    }`;
  }

  // Clipping indicator
  if (reg.clipEl) {
    let cs = clipState.get(id);
    if (!cs) {
      cs = { active: false, timeout: null };
      clipState.set(id, cs);
    }

    if (level.clipping && !cs.active) {
      cs.active = true;
      reg.clipEl.className = 'absolute rounded-t bg-red-500 animate-pulse';
      if (cs.timeout) clearTimeout(cs.timeout);
      cs.timeout = setTimeout(() => {
        cs!.active = false;
        if (reg.clipEl) {
          reg.clipEl.className = 'absolute rounded-t bg-transparent';
        }
      }, 1000);
    }
  }
}

// ============================================================================
// Auto-start / auto-stop
// ============================================================================

function startLoop(): void {
  if (rafId !== null) return;
  lastRenderTime = 0;
  lastDomTime = 0;
  lastVersion = -1;
  rafId = requestAnimationFrame(loop);
}

function stopLoop(): void {
  if (rafId !== null) {
    cancelAnimationFrame(rafId);
    rafId = null;
  }
}

// ============================================================================
// Public API
// ============================================================================

export function registerMeter(id: string, reg: MeterRegistration): void {
  meters.set(id, reg);
  if (meters.size === 1) {
    startLoop();
  }
}

export function unregisterMeter(id: string): void {
  meters.delete(id);
  const cs = clipState.get(id);
  if (cs?.timeout) clearTimeout(cs.timeout);
  clipState.delete(id);
  if (meters.size === 0) {
    stopLoop();
  }
}

export function pauseCoordinator(): void {
  paused = true;
}

export function resumeCoordinator(): void {
  paused = false;
}
