/**
 * Studio Store
 * Manages Studio Mode state with Preview/Program panes
 */
import { create } from 'zustand';
import { useProfileStore } from './profileStore';
import { useTransitionStore, getEffectiveTransition } from './transitionStore';
import type { SceneTransition } from '@/types/scene';

interface StudioState {
  /** Whether Studio Mode is enabled */
  enabled: boolean;
  /** Scene ID shown in Preview pane (editable) */
  previewSceneId: string | null;
  /** Scene ID shown in Program pane (live) */
  programSceneId: string | null;

  /**
   * Toggle Studio Mode on/off
   */
  toggleStudioMode: () => void;

  /**
   * Enable Studio Mode
   */
  enableStudioMode: () => void;

  /**
   * Disable Studio Mode
   */
  disableStudioMode: () => void;

  /**
   * Set the Preview scene (loads scene into Preview pane)
   */
  setPreviewScene: (sceneId: string) => void;

  /**
   * Execute TAKE - push Preview to Program with transition
   */
  executeTake: (transition?: SceneTransition) => Promise<void>;

  /**
   * Sync studio state with profile's active scene
   */
  syncWithProfile: (activeSceneId: string | null) => void;
}

export const useStudioStore = create<StudioState>((set, get) => ({
  enabled: false,
  previewSceneId: null,
  programSceneId: null,

  toggleStudioMode: () => {
    const { enabled } = get();
    if (enabled) {
      get().disableStudioMode();
    } else {
      get().enableStudioMode();
    }
  },

  enableStudioMode: () => {
    // Get current profile's active scene
    const profile = useProfileStore.getState().current;
    const activeSceneId = profile?.activeSceneId || profile?.scenes[0]?.id || null;

    set({
      enabled: true,
      programSceneId: activeSceneId,
      previewSceneId: activeSceneId,
    });
  },

  disableStudioMode: () => {
    set({
      enabled: false,
      previewSceneId: null,
      programSceneId: null,
    });
  },

  setPreviewScene: (sceneId) => {
    set({ previewSceneId: sceneId });
  },

  executeTake: async (overrideTransition) => {
    const { previewSceneId, programSceneId, enabled } = get();

    // Can't take if not in studio mode or preview equals program
    if (!enabled || !previewSceneId || previewSceneId === programSceneId) {
      return;
    }

    // Get profile and scene data
    const profile = useProfileStore.getState().current;
    if (!profile) return;

    const previewScene = profile.scenes.find((s) => s.id === previewSceneId);
    if (!previewScene) return;

    // Get effective transition
    const transition =
      overrideTransition ||
      getEffectiveTransition(previewScene.transitionIn, profile.defaultTransition);

    // Start transition animation
    if (programSceneId) {
      useTransitionStore.getState().startTransition(programSceneId, previewSceneId, transition);
    }

    // Wait for transition to complete (or instant for cut)
    const duration = transition.type === 'cut' ? 0 : transition.durationMs;

    await new Promise((resolve) => setTimeout(resolve, duration));

    // Update program to match preview
    set({ programSceneId: previewSceneId });

    // Also update the profile's active scene
    useProfileStore.getState().setCurrentActiveScene(previewSceneId);
  },

  syncWithProfile: (activeSceneId) => {
    const { enabled, programSceneId, previewSceneId } = get();

    if (!enabled) return;

    // If program scene was deleted, update to new active scene
    if (programSceneId && activeSceneId && programSceneId !== activeSceneId) {
      set({
        programSceneId: activeSceneId,
        previewSceneId: previewSceneId || activeSceneId,
      });
    }
  },
}));
