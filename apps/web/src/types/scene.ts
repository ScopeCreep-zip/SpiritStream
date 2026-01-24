/**
 * Scene types for multi-input streaming
 * Mirrors server/src/models/scene.rs
 */

/**
 * Scene - composition of sources with layout and audio mixing
 */
export interface Scene {
  id: string;
  name: string;
  canvasWidth: number;
  canvasHeight: number;
  layers: SourceLayer[];
  audioMixer: AudioMixer;
}

/**
 * SourceLayer - a source instance positioned within a scene
 */
export interface SourceLayer {
  id: string;
  sourceId: string;
  visible: boolean;
  locked: boolean;
  transform: Transform;
  zIndex: number;
}

/**
 * Transform - position and size of a layer on the canvas
 */
export interface Transform {
  x: number;
  y: number;
  width: number;
  height: number;
  rotation: number;
  crop?: Crop;
}

/**
 * Crop - pixels to remove from source edges
 */
export interface Crop {
  top: number;
  bottom: number;
  left: number;
  right: number;
}

/**
 * AudioMixer - audio mixing configuration for a scene
 */
export interface AudioMixer {
  masterVolume: number;
  tracks: AudioTrack[];
}

/**
 * AudioTrack - audio settings for a single source in the mixer
 */
export interface AudioTrack {
  sourceId: string;
  volume: number;
  muted: boolean;
  solo: boolean;
}

/**
 * Factory function for creating a default scene
 */
export function createDefaultScene(name = 'Main'): Scene {
  return {
    id: crypto.randomUUID(),
    name,
    canvasWidth: 1920,
    canvasHeight: 1080,
    layers: [],
    audioMixer: createDefaultAudioMixer(),
  };
}

/**
 * Factory function for creating a default transform (fullscreen)
 */
export function createDefaultTransform(
  canvasWidth = 1920,
  canvasHeight = 1080
): Transform {
  return {
    x: 0,
    y: 0,
    width: canvasWidth,
    height: canvasHeight,
    rotation: 0,
  };
}

/**
 * Factory function for creating a centered transform at a specific size
 */
export function createCenteredTransform(
  width: number,
  height: number,
  canvasWidth = 1920,
  canvasHeight = 1080
): Transform {
  return {
    x: Math.round((canvasWidth - width) / 2),
    y: Math.round((canvasHeight - height) / 2),
    width,
    height,
    rotation: 0,
  };
}

/**
 * Factory function for creating a PiP (picture-in-picture) transform
 */
export function createPipTransform(
  position: 'topLeft' | 'topRight' | 'bottomLeft' | 'bottomRight' = 'bottomRight',
  canvasWidth = 1920,
  canvasHeight = 1080,
  pipScale = 0.25,
  margin = 20
): Transform {
  const pipWidth = Math.round(canvasWidth * pipScale);
  const pipHeight = Math.round(canvasHeight * pipScale);

  let x: number;
  let y: number;

  switch (position) {
    case 'topLeft':
      x = margin;
      y = margin;
      break;
    case 'topRight':
      x = canvasWidth - pipWidth - margin;
      y = margin;
      break;
    case 'bottomLeft':
      x = margin;
      y = canvasHeight - pipHeight - margin;
      break;
    case 'bottomRight':
      x = canvasWidth - pipWidth - margin;
      y = canvasHeight - pipHeight - margin;
      break;
  }

  return {
    x,
    y,
    width: pipWidth,
    height: pipHeight,
    rotation: 0,
  };
}

/**
 * Factory function for creating a default layer
 */
export function createDefaultLayer(
  sourceId: string,
  transform?: Transform,
  zIndex = 0
): SourceLayer {
  return {
    id: crypto.randomUUID(),
    sourceId,
    visible: true,
    locked: false,
    transform: transform ?? createDefaultTransform(),
    zIndex,
  };
}

/**
 * Factory function for creating a default audio mixer
 */
export function createDefaultAudioMixer(): AudioMixer {
  return {
    masterVolume: 1.0,
    tracks: [],
  };
}

/**
 * Factory function for creating an audio track
 */
export function createDefaultAudioTrack(sourceId: string): AudioTrack {
  return {
    sourceId,
    volume: 1.0,
    muted: false,
    solo: false,
  };
}

/**
 * Helper to add a source to a scene as a fullscreen layer
 */
export function addFullscreenLayerToScene(scene: Scene, sourceId: string): Scene {
  const maxZIndex = Math.max(0, ...scene.layers.map((l) => l.zIndex));
  const newLayer = createDefaultLayer(
    sourceId,
    createDefaultTransform(scene.canvasWidth, scene.canvasHeight),
    maxZIndex + 1
  );

  return {
    ...scene,
    layers: [...scene.layers, newLayer],
    audioMixer: {
      ...scene.audioMixer,
      tracks: [...scene.audioMixer.tracks, createDefaultAudioTrack(sourceId)],
    },
  };
}

/**
 * Helper to reorder layers by z-index
 */
export function sortLayersByZIndex(layers: SourceLayer[]): SourceLayer[] {
  return [...layers].sort((a, b) => a.zIndex - b.zIndex);
}

/**
 * Helper to update layer order (move up/down in stack)
 */
export function reorderLayers(
  layers: SourceLayer[],
  layerId: string,
  direction: 'up' | 'down' | 'top' | 'bottom'
): SourceLayer[] {
  const sorted = sortLayersByZIndex(layers);
  const currentIndex = sorted.findIndex((l) => l.id === layerId);

  if (currentIndex === -1) return layers;

  let newIndex: number;
  switch (direction) {
    case 'up':
      newIndex = Math.min(sorted.length - 1, currentIndex + 1);
      break;
    case 'down':
      newIndex = Math.max(0, currentIndex - 1);
      break;
    case 'top':
      newIndex = sorted.length - 1;
      break;
    case 'bottom':
      newIndex = 0;
      break;
  }

  if (newIndex === currentIndex) return layers;

  // Reassign z-indices
  const reordered = [...sorted];
  const [moved] = reordered.splice(currentIndex, 1);
  reordered.splice(newIndex, 0, moved);

  return reordered.map((layer, index) => ({
    ...layer,
    zIndex: index,
  }));
}

/**
 * Common canvas resolutions
 */
export const CANVAS_PRESETS = [
  { label: '1080p (1920x1080)', width: 1920, height: 1080 },
  { label: '720p (1280x720)', width: 1280, height: 720 },
  { label: '4K (3840x2160)', width: 3840, height: 2160 },
  { label: '1440p (2560x1440)', width: 2560, height: 1440 },
  { label: 'Vertical 1080 (1080x1920)', width: 1080, height: 1920 },
  { label: 'Square (1080x1080)', width: 1080, height: 1080 },
] as const;
