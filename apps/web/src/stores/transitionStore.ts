/**
 * Transition Store
 * Manages scene transition state and animations
 */
import { create } from 'zustand';
import type { SceneTransition } from '@/types/scene';
import { DEFAULT_TRANSITION } from '@/types/scene';

interface TransitionState {
  /** Whether a transition is currently in progress */
  isTransitioning: boolean;
  /** The scene we're transitioning from */
  fromSceneId: string | null;
  /** The scene we're transitioning to */
  toSceneId: string | null;
  /** Current transition configuration */
  currentTransition: SceneTransition | null;
  /** Progress of the transition (0-1) */
  progress: number;

  /**
   * Start a transition between scenes
   */
  startTransition: (
    fromSceneId: string,
    toSceneId: string,
    transition: SceneTransition
  ) => void;

  /**
   * End the current transition
   */
  endTransition: () => void;

  /**
   * Update transition progress
   */
  setProgress: (progress: number) => void;
}

export const useTransitionStore = create<TransitionState>((set, get) => ({
  isTransitioning: false,
  fromSceneId: null,
  toSceneId: null,
  currentTransition: null,
  progress: 0,

  startTransition: (fromSceneId, toSceneId, transition) => {
    // If it's a cut transition, don't animate
    if (transition.type === 'cut') {
      return;
    }

    set({
      isTransitioning: true,
      fromSceneId,
      toSceneId,
      currentTransition: transition,
      progress: 0,
    });

    // Auto-end transition after duration
    setTimeout(() => {
      const state = get();
      if (state.isTransitioning && state.toSceneId === toSceneId) {
        get().endTransition();
      }
    }, transition.durationMs);
  },

  endTransition: () => {
    set({
      isTransitioning: false,
      fromSceneId: null,
      toSceneId: null,
      currentTransition: null,
      progress: 1,
    });
  },

  setProgress: (progress) => {
    set({ progress: Math.min(1, Math.max(0, progress)) });
  },
}));

/**
 * Get effective transition for a scene
 * Falls back to default if scene has no override
 */
export function getEffectiveTransition(
  sceneTransition?: SceneTransition,
  defaultTransition?: SceneTransition
): SceneTransition {
  if (sceneTransition) {
    return sceneTransition;
  }
  if (defaultTransition) {
    return defaultTransition;
  }
  return DEFAULT_TRANSITION;
}
