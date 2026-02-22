/**
 * Source Type Registry
 *
 * Single source of truth for all source type metadata. Replaces scattered
 * switch statements and factory functions with a declarative registry.
 *
 * Previously duplicated across:
 * - types/source.ts: 14 factory functions, sourceHasVideo/Audio switches, label/icon switches
 * - components/modals/AddSourceModal.tsx: type selection switch
 */

import type {
  SourceType,
  Source,
  RtmpSource,
  MediaFileSource,
  ScreenCaptureSource,
  WindowCaptureSource,
  CaptureCardSource,
  MediaPlaylistSource,
  GameCaptureSource,
  NDISource,
} from '@/types/source';

// ---------------------------------------------------------------------------
// Registry types
// ---------------------------------------------------------------------------

export type SourceCategory = 'capture' | 'media' | 'overlay' | 'composition';

export interface SourceTypeEntry {
  /** Source type discriminator */
  type: SourceType;
  /** Human-readable label */
  label: string;
  /** Lucide icon name */
  icon: string;
  /** Grouping category for the "Add Source" modal */
  category: SourceCategory;
  /** Does this source produce video? (static or dynamic check) */
  hasVideo: boolean | ((s: Source) => boolean);
  /** Does this source produce audio? (static or dynamic check) */
  hasAudio: boolean | ((s: Source) => boolean);
  /** Does adding this source require device discovery first? */
  needsDevice: boolean;
  /** Is this source rendered client-side (no WebRTC/backend capture needed)? */
  clientSide: boolean;
  /** Default config values (merged with id/name/type at creation time) */
  defaults: () => Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

export const SOURCE_REGISTRY: Record<SourceType, SourceTypeEntry> = {
  rtmp: {
    type: 'rtmp',
    label: 'RTMP Input',
    icon: 'Radio',
    category: 'capture',
    hasVideo: true,
    hasAudio: (s) => (s as RtmpSource).captureAudio,
    needsDevice: false,
    clientSide: false,
    defaults: () => ({
      bindAddress: '0.0.0.0',
      port: 1935,
      application: 'live',
      captureAudio: true,
    }),
  },

  camera: {
    type: 'camera',
    label: 'Camera',
    icon: 'Camera',
    category: 'capture',
    hasVideo: true,
    // Camera audio comes from linked AudioDeviceSource, not the camera itself
    hasAudio: false,
    needsDevice: true,
    clientSide: false,
    defaults: () => ({
      deviceId: '',
      captureAudio: false,
    }),
  },

  screenCapture: {
    type: 'screenCapture',
    label: 'Screen Capture',
    icon: 'Monitor',
    category: 'capture',
    hasVideo: true,
    hasAudio: (s) => (s as ScreenCaptureSource).captureAudio,
    needsDevice: true,
    clientSide: false,
    defaults: () => ({
      displayId: '',
      captureCursor: true,
      captureAudio: false,
      fps: 30,
      captureResolution: '1080p',
    }),
  },

  windowCapture: {
    type: 'windowCapture',
    label: 'Window Capture',
    icon: 'AppWindow',
    category: 'capture',
    hasVideo: true,
    hasAudio: (s) => (s as WindowCaptureSource).captureAudio,
    needsDevice: true,
    clientSide: false,
    defaults: () => ({
      windowId: '',
      windowTitle: '',
      captureCursor: true,
      fps: 30,
      captureAudio: false,
      captureResolution: '1080p',
    }),
  },

  gameCapture: {
    type: 'gameCapture',
    label: 'Game Capture',
    icon: 'Gamepad2',
    category: 'capture',
    hasVideo: true,
    hasAudio: (s) => (s as GameCaptureSource).captureAudio,
    needsDevice: false,
    clientSide: false,
    defaults: () => ({
      targetType: 'any',
      captureMode: 'auto',
      captureCursor: false,
      antiCheatHook: false,
      fps: 60,
      captureAudio: false,
    }),
  },

  captureCard: {
    type: 'captureCard',
    label: 'Capture Card',
    icon: 'Usb',
    category: 'capture',
    hasVideo: true,
    hasAudio: (s) => (s as CaptureCardSource).captureAudio,
    needsDevice: true,
    clientSide: false,
    defaults: () => ({
      deviceId: '',
      captureAudio: true,
    }),
  },

  ndi: {
    type: 'ndi',
    label: 'NDI Source',
    icon: 'Network',
    category: 'capture',
    hasVideo: true,
    hasAudio: (s) => (s as NDISource).captureAudio,
    needsDevice: false,
    clientSide: false,
    defaults: () => ({
      sourceName: '',
      lowBandwidth: false,
      receiverName: 'SpiritStream',
      captureAudio: true,
    }),
  },

  audioDevice: {
    type: 'audioDevice',
    label: 'Audio Device',
    icon: 'Mic',
    category: 'capture',
    hasVideo: false,
    hasAudio: true,
    needsDevice: true,
    clientSide: false,
    defaults: () => ({
      deviceId: '',
    }),
  },

  mediaFile: {
    type: 'mediaFile',
    label: 'Media File',
    icon: 'Film',
    category: 'media',
    hasVideo: (s) => !(s as MediaFileSource).audioOnly,
    hasAudio: (s) => (s as MediaFileSource).captureAudio !== false,
    needsDevice: false,
    clientSide: false,
    defaults: () => ({
      filePath: '',
      loopPlayback: false,
      audioOnly: false,
      captureAudio: true,
    }),
  },

  mediaPlaylist: {
    type: 'mediaPlaylist',
    label: 'Media Playlist',
    icon: 'ListVideo',
    category: 'media',
    hasVideo: true,
    hasAudio: (s) => (s as MediaPlaylistSource).captureAudio,
    needsDevice: false,
    clientSide: false,
    defaults: () => ({
      items: [],
      currentItemIndex: 0,
      autoAdvance: true,
      shuffleMode: 'none',
      fadeBetweenItems: false,
      fadeDurationMs: 500,
      captureAudio: true,
    }),
  },

  color: {
    type: 'color',
    label: 'Color',
    icon: 'Palette',
    category: 'overlay',
    hasVideo: true,
    hasAudio: false,
    needsDevice: false,
    clientSide: true,
    defaults: () => ({
      color: '#7C3AED',
      opacity: 1.0,
    }),
  },

  text: {
    type: 'text',
    label: 'Text',
    icon: 'Type',
    category: 'overlay',
    hasVideo: true,
    hasAudio: false,
    needsDevice: false,
    clientSide: true,
    defaults: () => ({
      content: '',
      fontFamily: 'Arial',
      fontSize: 48,
      fontWeight: 'normal',
      fontStyle: 'normal',
      textColor: '#FFFFFF',
      backgroundColor: undefined,
      backgroundOpacity: 0.8,
      textAlign: 'center',
      lineHeight: 1.2,
      padding: 16,
      outline: { enabled: false, color: '#000000', width: 2 },
    }),
  },

  browser: {
    type: 'browser',
    label: 'Browser',
    icon: 'Globe',
    category: 'overlay',
    hasVideo: true,
    hasAudio: false,
    needsDevice: false,
    clientSide: true,
    defaults: () => ({
      url: '',
      width: 1920,
      height: 1080,
      customCss: undefined,
      refreshInterval: 0,
    }),
  },

  nestedScene: {
    type: 'nestedScene',
    label: 'Nested Scene',
    icon: 'Layers',
    category: 'composition',
    hasVideo: true,
    hasAudio: false,
    needsDevice: false,
    clientSide: true,
    defaults: () => ({
      referencedSceneId: '',
    }),
  },
};

// ---------------------------------------------------------------------------
// Derived helpers — replace switch statements throughout the codebase
// ---------------------------------------------------------------------------

/** Get the human-readable label for a source type */
export function getSourceTypeLabel(type: SourceType): string {
  return SOURCE_REGISTRY[type].label;
}

/** Get the Lucide icon name for a source type */
export function getSourceTypeIcon(type: SourceType): string {
  return SOURCE_REGISTRY[type].icon;
}

/** Check if a source instance has video output */
export function sourceHasVideo(source: Source): boolean {
  const entry = SOURCE_REGISTRY[source.type];
  return typeof entry.hasVideo === 'function'
    ? entry.hasVideo(source)
    : entry.hasVideo;
}

/** Check if a source instance has audio output */
export function sourceHasAudio(source: Source): boolean {
  const entry = SOURCE_REGISTRY[source.type];
  return typeof entry.hasAudio === 'function'
    ? entry.hasAudio(source)
    : entry.hasAudio;
}

/** Check if a source renders client-side (no backend capture/WebRTC needed) */
export function isClientSideSource(source: Source): boolean {
  return SOURCE_REGISTRY[source.type].clientSide;
}

/**
 * Create a source with default values for its type.
 *
 * Replaces the 14 individual `createDefault*Source()` factory functions.
 * Pass `overrides` to customize specific fields (e.g., deviceId, displayId).
 */
export function createDefaultSource<T extends Source = Source>(
  type: SourceType,
  overrides: Partial<Record<string, unknown>> = {},
): T {
  const entry = SOURCE_REGISTRY[type];
  return {
    type,
    id: crypto.randomUUID(),
    name: overrides.name ?? entry.label,
    ...entry.defaults(),
    ...overrides,
  } as unknown as T;
}

/** Get all source types in a given category */
export function getSourceTypesByCategory(
  category: SourceCategory,
): SourceTypeEntry[] {
  return Object.values(SOURCE_REGISTRY).filter(
    (entry) => entry.category === category,
  );
}

/** All source types as an ordered array (for iteration) */
export const ALL_SOURCE_TYPES: SourceTypeEntry[] =
  Object.values(SOURCE_REGISTRY);
