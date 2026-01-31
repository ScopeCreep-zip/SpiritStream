/**
 * Projector Store
 * Manages projector window state for fullscreen scene output
 */
import { create } from 'zustand';

interface ProjectorState {
  /** Currently projected scene ID */
  projectedSceneId: string | null;
  /** Profile name for the projected scene */
  projectedProfileName: string | null;
  /** Whether projector is active */
  isProjecting: boolean;
}

interface ProjectorActions {
  /** Set the projected scene */
  setProjectedScene: (profileName: string, sceneId: string) => void;
  /** Clear the projected scene */
  clearProjectedScene: () => void;
  /** Open projector in new window (for Tauri) */
  openProjectorWindow: (profileName: string, sceneId: string) => Promise<void>;
  /** Close projector window */
  closeProjectorWindow: () => Promise<void>;
  /** Internal helper for browser popup */
  openProjectorPopup: (profileName: string, sceneId: string) => void;
}

export const useProjectorStore = create<ProjectorState & ProjectorActions>((set, get) => ({
  // Initial state
  projectedSceneId: null,
  projectedProfileName: null,
  isProjecting: false,

  // Actions
  setProjectedScene: (profileName, sceneId) => {
    set({
      projectedProfileName: profileName,
      projectedSceneId: sceneId,
      isProjecting: true,
    });
  },

  clearProjectedScene: () => {
    set({
      projectedProfileName: null,
      projectedSceneId: null,
      isProjecting: false,
    });
  },

  openProjectorWindow: async (profileName, sceneId) => {
    // Check if we're in Tauri environment
    if (typeof window !== 'undefined' && '__TAURI__' in window) {
      try {
        const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');

        // Close existing projector if any
        const existing = await WebviewWindow.getByLabel('projector');
        if (existing) {
          await existing.close();
        }

        // Create new fullscreen projector window
        const projector = new WebviewWindow('projector', {
          url: `/projector?profileName=${encodeURIComponent(profileName)}&sceneId=${encodeURIComponent(sceneId)}`,
          fullscreen: true,
          decorations: false,
          alwaysOnTop: false,
          title: 'SpiritStream Projector',
        });

        // Set state
        set({
          projectedProfileName: profileName,
          projectedSceneId: sceneId,
          isProjecting: true,
        });

        // Listen for close
        projector.once('tauri://destroyed', () => {
          set({
            projectedProfileName: null,
            projectedSceneId: null,
            isProjecting: false,
          });
        });
      } catch (err) {
        console.error('[Projector] Failed to open window:', err);
        // Fallback to browser popup
        get().openProjectorPopup(profileName, sceneId);
      }
    } else {
      // Browser mode - open in new window/tab
      get().openProjectorPopup(profileName, sceneId);
    }
  },

  closeProjectorWindow: async () => {
    if (typeof window !== 'undefined' && '__TAURI__' in window) {
      try {
        const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
        const projector = await WebviewWindow.getByLabel('projector');
        if (projector) {
          await projector.close();
        }
      } catch (err) {
        console.error('[Projector] Failed to close window:', err);
      }
    }

    set({
      projectedProfileName: null,
      projectedSceneId: null,
      isProjecting: false,
    });
  },

  // Internal helper for browser popup
  openProjectorPopup: (profileName: string, sceneId: string) => {
    const url = `/projector?profileName=${encodeURIComponent(profileName)}&sceneId=${encodeURIComponent(sceneId)}`;
    const popup = window.open(
      url,
      'spiritstream-projector',
      'fullscreen=yes,menubar=no,toolbar=no,location=no,status=no'
    );

    if (popup) {
      set({
        projectedProfileName: profileName,
        projectedSceneId: sceneId,
        isProjecting: true,
      });

      // Check for close periodically
      const checkClosed = setInterval(() => {
        if (popup.closed) {
          clearInterval(checkClosed);
          set({
            projectedProfileName: null,
            projectedSceneId: null,
            isProjecting: false,
          });
        }
      }, 500);
    }
  },
}));

