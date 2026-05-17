// Server-tuned encoder preset matrix. Backend is the source of truth:
// hydrates from `GET /api/v1/system/encoders/presets` at app start, then
// exposes values via the mutable `encoderPresets` object. The hard-coded
// defaults below are first-paint fallbacks (and a safety net for tests
// that don't boot the backend).
//
// Replaces ~80 lines of hard-coded `*_VALUES` arrays in
// `apps/web/src/components/modals/OutputGroupModal.tsx`.

import { api } from '@/lib/client';
import { logger } from '@/lib/logger';
import type { EncoderKind, EncoderMeta } from '@spiritstream/types';

export const encoderPresets = {
  RESOLUTION_VALUES: ['1920x1080', '1280x720', '2560x1440', '3840x2160', '854x480'],
  FPS_VALUES: ['60', '30', '24', '25', '50'],
  AUDIO_BITRATE_VALUES: ['320k', '256k', '192k', '160k', '128k', '96k', '64k'],
  AUDIO_CHANNELS_VALUES: ['1', '2', '6', '8'],
  AUDIO_SAMPLE_RATE_VALUES: ['48000', '44100', '32000'],
  CONTAINER_FORMAT_VALUES: ['flv', 'mpegts', 'mp4'],
  H264_PROFILE_VALUES: ['baseline', 'main', 'high'],
  /** Per-codec preset lists. Keys: `libx264`, `libx265`, `nvenc`, `amf`. */
  PRESETS: {
    libx264: ['ultrafast', 'superfast', 'veryfast', 'faster', 'fast', 'medium', 'slow', 'slower', 'veryslow'],
    libx265: ['ultrafast', 'superfast', 'veryfast', 'faster', 'fast', 'medium', 'slow', 'slower', 'veryslow'],
    nvenc: ['p1', 'p2', 'p3', 'p4', 'p5', 'p6', 'p7'],
    amf: ['quality', 'balanced', 'speed'],
  } as Record<string, string[]>,
  /** Default preset per family — backend-authoritative. */
  DEFAULT_PRESETS: {
    libx264: 'veryfast',
    libx265: 'veryfast',
    nvenc: 'p4',
    amf: 'balanced',
  } as Record<string, string>,
};

/** Per-encoder metadata hydrated from `GET /api/v1/system/encoders`.
 *  Lookup by codec name (e.g. `h264_nvenc`) → kind + family. The
 *  backend is the single source of truth for "is this hardware?" and
 *  "which preset list applies?". */
export const encoderMetadata: Record<string, EncoderMeta> = {};

export async function hydrateEncoderPresets(): Promise<void> {
  try {
    const data = await api.system.encoderPresets();
    encoderPresets.RESOLUTION_VALUES = data.resolutions;
    encoderPresets.FPS_VALUES = data.fpsValues;
    encoderPresets.AUDIO_BITRATE_VALUES = data.audioBitrates;
    encoderPresets.AUDIO_CHANNELS_VALUES = data.audioChannels;
    encoderPresets.AUDIO_SAMPLE_RATE_VALUES = data.audioSampleRates;
    encoderPresets.CONTAINER_FORMAT_VALUES = data.containerFormats;
    encoderPresets.H264_PROFILE_VALUES = data.h264Profiles;
    encoderPresets.PRESETS = data.presets;
    encoderPresets.DEFAULT_PRESETS = data.defaultPresets;
  } catch (err) {
    logger.warn('[encoderPresets] hydrate failed; keeping defaults:', err);
  }
}

export async function hydrateEncoderMetadata(): Promise<void> {
  try {
    const data = await api.system.getEncoders();
    // Replace, don't merge — fresh hydration per process lifetime.
    for (const key of Object.keys(encoderMetadata)) delete encoderMetadata[key];
    Object.assign(encoderMetadata, data.metadata);
  } catch (err) {
    logger.warn('[encoderMetadata] hydrate failed; using empty map:', err);
  }
}

/** Lookup the kind (hardware/software/passthrough) for a codec. */
export function getEncoderKind(codec: string): EncoderKind {
  if (codec === 'copy') return 'passthrough';
  return encoderMetadata[codec]?.kind ?? 'software';
}

/** Pick the per-codec preset list. Uses backend-hydrated `family`
 *  metadata; falls back to empty when unknown. */
export function getPresetValues(codec: string): string[] {
  const family = encoderMetadata[codec]?.family ?? '';
  if (!family) return [];
  return encoderPresets.PRESETS[family] ?? [];
}

/** Default preset for a codec — backend-authoritative via family. */
export function getDefaultPreset(codec: string, presetValues: string[]): string {
  const family = encoderMetadata[codec]?.family ?? '';
  const fromBackend = family ? encoderPresets.DEFAULT_PRESETS[family] : undefined;
  if (fromBackend && presetValues.includes(fromBackend)) return fromBackend;
  if (presetValues.includes('veryfast')) return 'veryfast';
  return presetValues[0] || '';
}
