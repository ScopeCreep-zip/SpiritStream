/**
 * Filter Registry
 *
 * Single source of truth for audio and video filter metadata and defaults.
 * Replaces scattered factory functions and label switch statements.
 *
 * Previously duplicated in types/source.ts:
 * - 5 audio filter factory functions (createCompressorFilter, etc.)
 * - 9 video filter factory functions (createChromaKeyFilter, etc.)
 * - getAudioFilterLabel() switch
 * - getVideoFilterLabel() switch
 */

import type {
  AudioFilterType,
  AudioFilter,
  VideoFilterType,
  VideoFilter,
} from '@/types/source';

// ---------------------------------------------------------------------------
// Audio filter registry
// ---------------------------------------------------------------------------

interface AudioFilterEntry<T extends AudioFilter = AudioFilter> {
  type: AudioFilterType;
  label: string;
  defaults: () => Omit<T, 'id' | 'type' | 'enabled' | 'order'>;
}

export const AUDIO_FILTER_REGISTRY: Record<AudioFilterType, AudioFilterEntry> =
  {
    compressor: {
      type: 'compressor',
      label: 'Compressor',
      defaults: () => ({
        threshold: -20,
        ratio: 4,
        attack: 5,
        release: 50,
        outputGain: 0,
      }),
    },
    noiseGate: {
      type: 'noiseGate',
      label: 'Noise Gate',
      defaults: () => ({
        threshold: -40,
        attack: 5,
        hold: 100,
        release: 100,
      }),
    },
    noiseSuppression: {
      type: 'noiseSuppression',
      label: 'Noise Suppression',
      defaults: () => ({ level: 50 }),
    },
    gain: {
      type: 'gain',
      label: 'Gain',
      defaults: () => ({ gain: 0 }),
    },
    expander: {
      type: 'expander',
      label: 'Expander',
      defaults: () => ({
        threshold: -40,
        ratio: 2,
        attack: 5,
        release: 100,
      }),
    },
  };

// ---------------------------------------------------------------------------
// Video filter registry
// ---------------------------------------------------------------------------

interface VideoFilterEntry<T extends VideoFilter = VideoFilter> {
  type: VideoFilterType;
  label: string;
  defaults: () => Omit<T, 'id' | 'type' | 'enabled' | 'order'>;
}

export const VIDEO_FILTER_REGISTRY: Record<VideoFilterType, VideoFilterEntry> =
  {
    chromaKey: {
      type: 'chromaKey',
      label: 'Chroma Key',
      defaults: () => ({
        keyColor: '#00FF00',
        similarity: 400,
        smoothness: 80,
        keySpill: 100,
      }),
    },
    colorKey: {
      type: 'colorKey',
      label: 'Color Key',
      defaults: () => ({
        keyColor: '#00FF00',
        similarity: 400,
        smoothness: 80,
      }),
    },
    colorCorrection: {
      type: 'colorCorrection',
      label: 'Color Correction',
      defaults: () => ({
        brightness: 0,
        contrast: 0,
        saturation: 1,
        gamma: 1,
        hue: 0,
      }),
    },
    lut: {
      type: 'lut',
      label: 'LUT',
      defaults: () => ({ lutFile: '', intensity: 1 }),
    },
    blur: {
      type: 'blur',
      label: 'Blur',
      defaults: () => ({ blurType: 'gaussian' as const, size: 10 }),
    },
    sharpen: {
      type: 'sharpen',
      label: 'Sharpen',
      defaults: () => ({ amount: 1 }),
    },
    scroll: {
      type: 'scroll',
      label: 'Scroll',
      defaults: () => ({
        horizontalSpeed: 0,
        verticalSpeed: 50,
        loop: true,
      }),
    },
    mask: {
      type: 'mask',
      label: 'Image Mask',
      defaults: () => ({
        maskImage: '',
        maskType: 'alpha' as const,
        invert: false,
      }),
    },
    transform3d: {
      type: 'transform3d',
      label: '3D Transform',
      defaults: () => ({
        rotationX: 0,
        rotationY: 0,
        rotationZ: 0,
        perspective: 1000,
        positionX: 0,
        positionY: 0,
        positionZ: 0,
      }),
    },
  };

// ---------------------------------------------------------------------------
// Factory helpers
// ---------------------------------------------------------------------------

/** Create an audio filter with default values */
export function createAudioFilter<T extends AudioFilter = AudioFilter>(
  type: AudioFilterType,
): T {
  const entry = AUDIO_FILTER_REGISTRY[type];
  return {
    id: crypto.randomUUID(),
    type,
    enabled: true,
    order: 0,
    ...entry.defaults(),
  } as unknown as T;
}

/** Create a video filter with default values */
export function createVideoFilter<T extends VideoFilter = VideoFilter>(
  type: VideoFilterType,
): T {
  const entry = VIDEO_FILTER_REGISTRY[type];
  return {
    id: crypto.randomUUID(),
    type,
    enabled: true,
    order: 0,
    ...entry.defaults(),
  } as unknown as T;
}

/** Get human-readable label for an audio filter type */
export function getAudioFilterLabel(type: AudioFilterType): string {
  return AUDIO_FILTER_REGISTRY[type].label;
}

/** Get human-readable label for a video filter type */
export function getVideoFilterLabel(type: VideoFilterType): string {
  return VIDEO_FILTER_REGISTRY[type].label;
}
