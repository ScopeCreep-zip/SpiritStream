/**
 * Scene Validation Utilities
 * Prevents circular references in nested scenes
 */
import type { Scene } from '@/types/scene';
import type { Source, NestedSceneSource } from '@/types/source';

/**
 * Check if adding a nested scene reference would create a circular dependency
 *
 * @param scenes - All scenes in the profile
 * @param sources - All sources in the profile
 * @param currentSceneId - The scene we're adding the nested scene to
 * @param referencedSceneId - The scene we want to reference
 * @returns true if this would create a cycle, false if safe
 */
export function wouldCreateCycle(
  scenes: Scene[],
  sources: Source[],
  currentSceneId: string,
  referencedSceneId: string
): boolean {
  // Can't reference yourself
  if (currentSceneId === referencedSceneId) {
    return true;
  }

  // DFS to detect cycles
  const visited = new Set<string>();
  const stack = [referencedSceneId];

  while (stack.length > 0) {
    const sceneId = stack.pop()!;

    // Found a cycle back to the current scene
    if (sceneId === currentSceneId) {
      return true;
    }

    // Already visited this scene in this path
    if (visited.has(sceneId)) {
      continue;
    }
    visited.add(sceneId);

    // Find the scene
    const scene = scenes.find((s) => s.id === sceneId);
    if (!scene) {
      continue;
    }

    // Check all layers in this scene for nested scene sources
    for (const layer of scene.layers) {
      const source = sources.find((s) => s.id === layer.sourceId);
      if (source && source.type === 'nestedScene') {
        const nestedSource = source as NestedSceneSource;
        stack.push(nestedSource.referencedSceneId);
      }
    }
  }

  return false;
}

/**
 * Get all scenes that can be safely referenced from a given scene
 * (excludes scenes that would create a circular dependency)
 *
 * @param scenes - All scenes in the profile
 * @param sources - All sources in the profile
 * @param currentSceneId - The scene we're adding the nested scene to
 * @returns Array of scenes that can be safely referenced
 */
export function getAvailableNestedScenes(
  scenes: Scene[],
  sources: Source[],
  currentSceneId: string
): Scene[] {
  return scenes.filter((scene) => {
    // Can't reference the current scene
    if (scene.id === currentSceneId) {
      return false;
    }

    // Check if this would create a cycle
    return !wouldCreateCycle(scenes, sources, currentSceneId, scene.id);
  });
}

/**
 * Validate that a profile's nested scene references don't have cycles
 *
 * @param scenes - All scenes in the profile
 * @param sources - All sources in the profile
 * @returns Array of error messages if cycles detected, empty array if valid
 */
export function validateNestedScenes(
  scenes: Scene[],
  sources: Source[]
): string[] {
  const errors: string[] = [];

  for (const scene of scenes) {
    for (const layer of scene.layers) {
      const source = sources.find((s) => s.id === layer.sourceId);
      if (source && source.type === 'nestedScene') {
        const nestedSource = source as NestedSceneSource;

        // Check if referenced scene exists
        const referencedScene = scenes.find((s) => s.id === nestedSource.referencedSceneId);
        if (!referencedScene) {
          errors.push(
            `Scene "${scene.name}" references non-existent scene (ID: ${nestedSource.referencedSceneId})`
          );
          continue;
        }

        // Check for cycles (excluding the source we're checking from the check)
        // We need a modified check that starts from the referenced scene
        const sourcesWithoutCurrent = sources.filter((s) => s.id !== source.id);
        if (wouldCreateCycle(scenes, sourcesWithoutCurrent, scene.id, nestedSource.referencedSceneId)) {
          errors.push(
            `Circular reference detected: Scene "${scene.name}" -> "${referencedScene.name}" creates a cycle`
          );
        }
      }
    }
  }

  return errors;
}

/**
 * Get the depth of nested scene references (for rendering order/performance warnings)
 *
 * @param scenes - All scenes in the profile
 * @param sources - All sources in the profile
 * @param sceneId - The scene to check depth for
 * @returns Maximum nesting depth (0 = no nested scenes, 1 = one level, etc.)
 */
export function getNestedSceneDepth(
  scenes: Scene[],
  sources: Source[],
  sceneId: string,
  visited: Set<string> = new Set()
): number {
  if (visited.has(sceneId)) {
    return 0; // Cycle detected, stop recursion
  }
  visited.add(sceneId);

  const scene = scenes.find((s) => s.id === sceneId);
  if (!scene) {
    return 0;
  }

  let maxDepth = 0;

  for (const layer of scene.layers) {
    const source = sources.find((s) => s.id === layer.sourceId);
    if (source && source.type === 'nestedScene') {
      const nestedSource = source as NestedSceneSource;
      const depth = 1 + getNestedSceneDepth(scenes, sources, nestedSource.referencedSceneId, visited);
      maxDepth = Math.max(maxDepth, depth);
    }
  }

  return maxDepth;
}
