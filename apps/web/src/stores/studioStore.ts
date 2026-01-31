/**
 * Studio Store
 * Manages Studio Mode state with Preview/Program panes
 */
import { create } from 'zustand';
import { useProfileStore } from './profileStore';
import { useTransitionStore, getEffectiveTransition } from './transitionStore';
import type { SceneTransition } from '@/types/scene';
import { isTauri } from '@/lib/backend/env';

// Minimum dimensions for Studio Mode layout
// Width: Sources panel (224) + Preview (300) + Controls (80) + Program (300) + Properties (224) + gaps (~72) = ~1200px
// Height: Top bar (70) + Canvases (400) + Scene bar (70) + Audio mixer (200) + gaps (80) = ~820px
const STUDIO_MODE_MIN_WIDTH = 1200;
const STUDIO_MODE_MIN_HEIGHT = 850;

/**
 * Ensure window is large enough for Studio Mode
 * Only runs in Tauri environment
 */
async function ensureWindowSizeForStudioMode(): Promise<void> {
  if (!isTauri()) return;

  try {
    // Dynamically import Tauri window API to avoid issues in non-Tauri environments
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    const window = getCurrentWindow();

    // Get current window size
    const currentSize = await window.innerSize();
    const currentWidth = currentSize.width;
    const currentHeight = currentSize.height;

    // Calculate new dimensions - only increase if needed
    const newWidth = Math.max(currentWidth, STUDIO_MODE_MIN_WIDTH);
    const newHeight = Math.max(currentHeight, STUDIO_MODE_MIN_HEIGHT);

    // Only resize if needed
    if (newWidth > currentWidth || newHeight > currentHeight) {
      const { LogicalSize } = await import('@tauri-apps/api/dpi');
      await window.setSize(new LogicalSize(newWidth, newHeight));
    }
  } catch (err) {
    console.warn('[StudioStore] Failed to resize window:', err);
  }
}

interface StudioState {
  /** Whether Studio Mode is enabled */
  enabled: boolean;
  /** Scene ID shown in Preview pane (editable) */
  previewSceneId: string | null;
  /** Scene ID shown in Program pane (live) */
  programSceneId: string | null;
  /** Whether to swap preview/program after TAKE transition */
  swapAfterTransition: boolean;
  /** T-bar progress (0 = fully on Program, 1 = fully on Preview) */
  tBarProgress: number;
  /** Whether the T-bar is currently being dragged */
  tBarDragging: boolean;

  /**
   * Toggle Studio Mode on/off
   */
  toggleStudioMode: () => void;

  /**
   * Set swap after transition setting
   */
  setSwapAfterTransition: (enabled: boolean) => void;

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

  /**
   * Set T-bar progress (0-1)
   * Used for manual transition control
   */
  setTBarProgress: (progress: number) => void;

  /**
   * Start T-bar dragging
   */
  startTBarDrag: () => void;

  /**
   * End T-bar dragging - if progress >= 0.5, complete transition
   */
  endTBarDrag: () => void;
}

export const useStudioStore = create<StudioState>((set, get) => ({
  enabled: false,
  previewSceneId: null,
  programSceneId: null,
  swapAfterTransition: false,
  tBarProgress: 0,
  tBarDragging: false,

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

    // Ensure window is large enough for Studio Mode
    ensureWindowSizeForStudioMode();

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

  setSwapAfterTransition: (enabled) => {
    set({ swapAfterTransition: enabled });
  },

  setPreviewScene: (sceneId) => {
    set({ previewSceneId: sceneId });
  },

  executeTake: async (overrideTransition) => {
    const { previewSceneId, programSceneId, enabled, swapAfterTransition } = get();

    // Can't take if not in studio mode or preview equals program
    if (!enabled || !previewSceneId || previewSceneId === programSceneId) {
      return;
    }

    // Get profile and scene data
    const profile = useProfileStore.getState().current;
    if (!profile) return;

    const previewScene = profile.scenes.find((s) => s.id === previewSceneId);
    if (!previewScene) return;

    // Store old program scene for potential swap
    const oldProgramSceneId = programSceneId;

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

    // Update program to match preview, and optionally swap preview to old program
    if (swapAfterTransition && oldProgramSceneId) {
      set({
        programSceneId: previewSceneId,
        previewSceneId: oldProgramSceneId,
      });
    } else {
      set({ programSceneId: previewSceneId });
    }

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

  setTBarProgress: (progress) => {
    // Clamp to 0-1
    const clampedProgress = Math.max(0, Math.min(1, progress));
    set({ tBarProgress: clampedProgress });
  },

  startTBarDrag: () => {
    set({ tBarDragging: true });
  },

  endTBarDrag: () => {
    const { tBarProgress, previewSceneId, programSceneId, swapAfterTransition } = get();

    // If progress is >= 0.5 (or reached 1), complete the transition
    if (tBarProgress >= 0.5) {
      // Store old program for swap
      const oldProgramSceneId = programSceneId;

      // Complete the transition instantly (since T-bar controlled the visual blend)
      if (swapAfterTransition && oldProgramSceneId) {
        set({
          programSceneId: previewSceneId,
          previewSceneId: oldProgramSceneId,
          tBarProgress: 0,
          tBarDragging: false,
        });
      } else {
        set({
          programSceneId: previewSceneId,
          tBarProgress: 0,
          tBarDragging: false,
        });
      }

      // Update the profile's active scene
      if (previewSceneId) {
        useProfileStore.getState().setCurrentActiveScene(previewSceneId);
      }
    } else {
      // Snap back to 0 (cancel the transition)
      set({
        tBarProgress: 0,
        tBarDragging: false,
      });
    }
  },
}));
