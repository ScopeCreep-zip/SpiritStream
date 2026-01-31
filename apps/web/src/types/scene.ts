/**
 * Scene types for multi-input streaming
 * Mirrors server/src/models/scene.rs
 */
import type { VideoFilter, AudioFilter } from './source';

/**
 * Transition types for scene switching
 */
export type TransitionType =
  | 'cut' // Instant (no animation)
  | 'fade' // Fade through black
  | 'fadeToColor' // Fade through a specific color
  | 'crossfade' // Dissolve between scenes
  | 'slideLeft'
  | 'slideRight'
  | 'slideUp'
  | 'slideDown'
  | 'wipeLeft'
  | 'wipeRight'
  | 'wipeUp'
  | 'wipeDown';

/**
 * Transition configuration
 */
export interface SceneTransition {
  type: TransitionType;
  durationMs: number; // 100-2000, default 300
  /** Color for fadeToColor transition (hex string, e.g. '#000000') */
  color?: string;
}

/**
 * Scene - composition of sources with layout and audio mixing
 */
export interface Scene {
  id: string;
  name: string;
  canvasWidth: number;
  canvasHeight: number;
  layers: SourceLayer[];
  /** Layer groups for organizing layers */
  groups?: LayerGroup[];
  audioMixer: AudioMixer;
  /** Override transition for this scene (optional) */
  transitionIn?: SceneTransition;
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
  /** Video filters applied to this layer */
  videoFilters?: VideoFilter[];
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
 * LayerGroup - a group of layers that can be moved/transformed together
 */
export interface LayerGroup {
  id: string;
  name: string;
  /** IDs of layers in this group (order matters for display) */
  layerIds: string[];
  /** Whether the group is visible (toggles all children) */
  visible: boolean;
  /** Whether the group is locked (locks all children) */
  locked: boolean;
  /** Whether the group is collapsed in the UI */
  collapsed: boolean;
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
  /** Audio filters applied to this track */
  audioFilters?: AudioFilter[];
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

/**
 * Default transition configuration
 */
export const DEFAULT_TRANSITION: SceneTransition = {
  type: 'fade',
  durationMs: 300,
};

/**
 * Factory function for creating a transition
 */
export function createDefaultTransitionConfig(
  type: TransitionType = 'fade',
  durationMs: number = 300
): SceneTransition {
  return { type, durationMs };
}

/**
 * Get human-readable label for transition type
 */
export function getTransitionTypeLabel(type: TransitionType): string {
  switch (type) {
    case 'cut':
      return 'Cut';
    case 'fade':
      return 'Fade';
    case 'fadeToColor':
      return 'Fade to Color';
    case 'crossfade':
      return 'Crossfade';
    case 'slideLeft':
      return 'Slide Left';
    case 'slideRight':
      return 'Slide Right';
    case 'slideUp':
      return 'Slide Up';
    case 'slideDown':
      return 'Slide Down';
    case 'wipeLeft':
      return 'Wipe Left';
    case 'wipeRight':
      return 'Wipe Right';
    case 'wipeUp':
      return 'Wipe Up';
    case 'wipeDown':
      return 'Wipe Down';
    default:
      return type;
  }
}

/**
 * All available transition types
 */
export const TRANSITION_TYPES: TransitionType[] = [
  'cut',
  'fade',
  'fadeToColor',
  'crossfade',
  'slideLeft',
  'slideRight',
  'slideUp',
  'slideDown',
  'wipeLeft',
  'wipeRight',
  'wipeUp',
  'wipeDown',
];

/**
 * Default color for fadeToColor transition (black)
 */
export const DEFAULT_FADE_COLOR = '#000000';

/**
 * Factory function for creating a layer group
 */
export function createLayerGroup(name: string, layerIds: string[] = []): LayerGroup {
  return {
    id: crypto.randomUUID(),
    name,
    layerIds,
    visible: true,
    locked: false,
    collapsed: false,
  };
}

/**
 * Get the group that contains a layer, if any
 */
export function getLayerGroup(scene: Scene, layerId: string): LayerGroup | undefined {
  return scene.groups?.find((g) => g.layerIds.includes(layerId));
}

/**
 * Check if a layer is in any group
 */
export function isLayerInGroup(scene: Scene, layerId: string): boolean {
  return scene.groups?.some((g) => g.layerIds.includes(layerId)) ?? false;
}

/**
 * Get all ungrouped layers in a scene
 */
export function getUngroupedLayers(scene: Scene): SourceLayer[] {
  const groupedLayerIds = new Set(scene.groups?.flatMap((g) => g.layerIds) ?? []);
  return scene.layers.filter((l) => !groupedLayerIds.has(l.id));
}

/**
 * Add a layer to a group
 */
export function addLayerToGroup(
  scene: Scene,
  groupId: string,
  layerId: string
): Scene {
  if (!scene.groups) return scene;

  // Remove from any existing group first
  const updatedGroups = scene.groups.map((g) => ({
    ...g,
    layerIds: g.layerIds.filter((id) => id !== layerId),
  }));

  // Add to target group
  return {
    ...scene,
    groups: updatedGroups.map((g) =>
      g.id === groupId ? { ...g, layerIds: [...g.layerIds, layerId] } : g
    ),
  };
}

/**
 * Remove a layer from its group
 */
export function removeLayerFromGroup(scene: Scene, layerId: string): Scene {
  if (!scene.groups) return scene;

  return {
    ...scene,
    groups: scene.groups.map((g) => ({
      ...g,
      layerIds: g.layerIds.filter((id) => id !== layerId),
    })),
  };
}

/**
 * Create a group from selected layers
 */
export function createGroupFromLayers(
  scene: Scene,
  layerIds: string[],
  groupName: string
): Scene {
  // Remove layers from any existing groups
  let updatedGroups = scene.groups?.map((g) => ({
    ...g,
    layerIds: g.layerIds.filter((id) => !layerIds.includes(id)),
  })) ?? [];

  // Create new group
  const newGroup = createLayerGroup(groupName, layerIds);

  return {
    ...scene,
    groups: [...updatedGroups, newGroup],
  };
}

/**
 * Delete a group (layers remain, just ungrouped)
 */
export function deleteGroup(scene: Scene, groupId: string): Scene {
  if (!scene.groups) return scene;

  return {
    ...scene,
    groups: scene.groups.filter((g) => g.id !== groupId),
  };
}

/**
 * Ungroup all layers in a group (alias for deleteGroup)
 */
export function ungroupLayers(scene: Scene, groupId: string): Scene {
  return deleteGroup(scene, groupId);
}

/**
 * Toggle group visibility (affects all layers in group)
 */
export function toggleGroupVisibility(scene: Scene, groupId: string): Scene {
  const group = scene.groups?.find((g) => g.id === groupId);
  if (!group || !scene.groups) return scene;

  const newVisibility = !group.visible;

  return {
    ...scene,
    groups: scene.groups.map((g) =>
      g.id === groupId ? { ...g, visible: newVisibility } : g
    ),
    // Also update layers' visibility
    layers: scene.layers.map((l) =>
      group.layerIds.includes(l.id) ? { ...l, visible: newVisibility } : l
    ),
  };
}

/**
 * Toggle group lock state (affects all layers in group)
 */
export function toggleGroupLock(scene: Scene, groupId: string): Scene {
  const group = scene.groups?.find((g) => g.id === groupId);
  if (!group || !scene.groups) return scene;

  const newLocked = !group.locked;

  return {
    ...scene,
    groups: scene.groups.map((g) =>
      g.id === groupId ? { ...g, locked: newLocked } : g
    ),
    // Also update layers' lock state
    layers: scene.layers.map((l) =>
      group.layerIds.includes(l.id) ? { ...l, locked: newLocked } : l
    ),
  };
}

/**
 * Toggle group collapsed state in UI
 */
export function toggleGroupCollapsed(scene: Scene, groupId: string): Scene {
  if (!scene.groups) return scene;

  return {
    ...scene,
    groups: scene.groups.map((g) =>
      g.id === groupId ? { ...g, collapsed: !g.collapsed } : g
    ),
  };
}

/**
 * Rename a group
 */
export function renameGroup(scene: Scene, groupId: string, newName: string): Scene {
  if (!scene.groups) return scene;

  return {
    ...scene,
    groups: scene.groups.map((g) =>
      g.id === groupId ? { ...g, name: newName } : g
    ),
  };
}
