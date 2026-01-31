/**
 * Projector Store
 * Manages multiple projector windows for fullscreen output
 * Supports OBS-parity features: scene, source, preview, program, multiview projectors
 */
import { create } from 'zustand';
import type {
  ProjectorType,
  ProjectorConfig,
  ProjectorInstance,
  MonitorInfo,
} from '@/types/projector';
import { buildProjectorUrl, generateProjectorId } from '@/types/projector';

interface ProjectorState {
  /** Map of active projector instances by ID */
  projectors: Map<string, ProjectorInstance>;
  /** Available monitors for fullscreen projection */
  monitors: MonitorInfo[];
  /** Whether monitor list has been fetched */
  monitorsLoaded: boolean;
}

interface ProjectorActions {
  /** Open a projector window */
  openProjector: (config: Omit<ProjectorConfig, 'id'>) => string;
  /** Close a specific projector by ID */
  closeProjector: (projectorId: string) => void;
  /** Close all projectors */
  closeAllProjectors: () => void;
  /** Close all projectors of a specific type */
  closeProjectorsByType: (type: ProjectorType) => void;
  /** Get projector by ID */
  getProjector: (projectorId: string) => ProjectorInstance | undefined;
  /** Get all projectors of a type */
  getProjectorsByType: (type: ProjectorType) => ProjectorInstance[];
  /** Check if any projector is active */
  hasActiveProjectors: () => boolean;
  /** Refresh available monitors */
  refreshMonitors: () => Promise<void>;
  /** Internal: open browser popup (synchronous to avoid popup blocker) */
  _openPopup: (url: string, projectorId: string) => Window | null;
  /** Internal: open Tauri window */
  _openTauriWindow: (config: ProjectorConfig) => Promise<void>;
  /** Internal: cleanup closed projector */
  _cleanupProjector: (projectorId: string) => void;

  // Legacy compatibility methods
  /** @deprecated Use openProjector instead */
  openProjectorWindow: (profileName: string, sceneId: string) => void;
  /** @deprecated Use closeAllProjectors instead */
  closeProjectorWindow: () => Promise<void>;
  /** @deprecated Check projectors.size > 0 instead */
  isProjecting: boolean;
}

export const useProjectorStore = create<ProjectorState & ProjectorActions>((set, get) => ({
  // Initial state
  projectors: new Map(),
  monitors: [],
  monitorsLoaded: false,

  // Legacy compatibility
  get isProjecting() {
    return get().projectors.size > 0;
  },

  openProjector: (config) => {
    const projectorId = generateProjectorId(config.type, config.targetId);
    const fullConfig: ProjectorConfig = { ...config, id: projectorId };

    // Check if we're in Tauri environment
    // In Tauri 2.x, use window.isTauri (introduced in 2.0.0-beta.16)
    // This is always available in Tauri webviews, unlike __TAURI__ which requires withGlobalTauri
    const isTauri = typeof window !== 'undefined' && (window as { isTauri?: boolean }).isTauri === true;

    console.log('[Projector] openProjector called:', {
      projectorId,
      type: config.type,
      isTauri,
      windowIsTauri: (window as { isTauri?: boolean }).isTauri,
    });

    if (isTauri) {
      // Tauri mode - use native WebviewWindow API for ALL windows
      // This bypasses browser popup blocking entirely since it's a native API
      console.log('[Projector] Using Tauri WebviewWindow API');

      const instance: ProjectorInstance = {
        config: fullConfig,
        windowRef: null,
        isActive: true,
      };

      set((state) => {
        const newProjectors = new Map(state.projectors);
        newProjectors.set(projectorId, instance);
        return { projectors: newProjectors };
      });

      // Open Tauri window asynchronously - this is safe because WebviewWindow
      // doesn't have the same popup blocking restrictions as window.open()
      get()._openTauriWindow(fullConfig).catch((err) => {
        console.error('[Projector] Failed to open Tauri window:', err);
        get()._cleanupProjector(projectorId);
      });

      return projectorId;
    }

    // Browser/HTTP mode - use window.open() with popup blocker workaround
    // CRITICAL: This must be called synchronously from a user gesture handler
    // to avoid popup blocking in Safari/WebKit
    console.log('[Projector] Using browser window.open() with about:blank workaround');

    const url = buildProjectorUrl(config);
    const windowRef = get()._openPopup(url, projectorId);

    // Only add to projectors map if popup actually opened
    if (!windowRef) {
      console.error('[Projector] Failed to open popup - blocked by browser');
      console.error('[Projector] Ensure openProjector is called synchronously from a click handler');
      return '';
    }

    const instance: ProjectorInstance = {
      config: fullConfig,
      windowRef,
      isActive: true,
    };

    set((state) => {
      const newProjectors = new Map(state.projectors);
      newProjectors.set(projectorId, instance);
      return { projectors: newProjectors };
    });

    // Monitor window close
    const checkClosed = setInterval(() => {
      if (windowRef.closed) {
        clearInterval(checkClosed);
        get()._cleanupProjector(projectorId);
      }
    }, 500);

    return projectorId;
  },

  closeProjector: (projectorId) => {
    const instance = get().projectors.get(projectorId);
    if (!instance) return;

    // Close the window
    if (instance.windowRef && !instance.windowRef.closed) {
      instance.windowRef.close();
    }

    get()._cleanupProjector(projectorId);
  },

  closeAllProjectors: () => {
    const { projectors } = get();
    projectors.forEach((instance) => {
      if (instance.windowRef && !instance.windowRef.closed) {
        instance.windowRef.close();
      }
    });
    set({ projectors: new Map() });
  },

  closeProjectorsByType: (type) => {
    const { projectors } = get();
    const toRemove: string[] = [];

    projectors.forEach((instance, id) => {
      if (instance.config.type === type) {
        if (instance.windowRef && !instance.windowRef.closed) {
          instance.windowRef.close();
        }
        toRemove.push(id);
      }
    });

    if (toRemove.length > 0) {
      set((state) => {
        const newProjectors = new Map(state.projectors);
        toRemove.forEach((id) => newProjectors.delete(id));
        return { projectors: newProjectors };
      });
    }
  },

  getProjector: (projectorId) => {
    return get().projectors.get(projectorId);
  },

  getProjectorsByType: (type) => {
    const result: ProjectorInstance[] = [];
    get().projectors.forEach((instance) => {
      if (instance.config.type === type) {
        result.push(instance);
      }
    });
    return result;
  },

  hasActiveProjectors: () => {
    return get().projectors.size > 0;
  },

  refreshMonitors: async () => {
    // In browser mode, we can only detect a rough approximation using window.screen
    // Full monitor detection requires Tauri or backend support
    const isTauri = typeof window !== 'undefined' && (window as { isTauri?: boolean }).isTauri === true;

    if (isTauri) {
      try {
        // Try to get monitors from Tauri
        const { availableMonitors, primaryMonitor } = await import('@tauri-apps/api/window');
        const monitors = await availableMonitors();
        const primary = await primaryMonitor();

        const monitorInfos: MonitorInfo[] = monitors.map((m, idx) => ({
          id: m.name || `monitor-${idx}`,
          name: m.name || `Display ${idx + 1}`,
          width: m.size.width,
          height: m.size.height,
          x: m.position.x,
          y: m.position.y,
          isPrimary: primary?.name === m.name,
          scaleFactor: m.scaleFactor,
        }));

        set({ monitors: monitorInfos, monitorsLoaded: true });
        return;
      } catch (err) {
        console.warn('[Projector] Failed to get Tauri monitors:', err);
      }
    }

    // Fallback: use window.screen for basic info
    const screenInfo: MonitorInfo = {
      id: 'primary',
      name: 'Primary Display',
      width: window.screen.width,
      height: window.screen.height,
      x: 0,
      y: 0,
      isPrimary: true,
      scaleFactor: window.devicePixelRatio,
    };

    set({ monitors: [screenInfo], monitorsLoaded: true });
  },

  // Internal helpers
  _openPopup: (url, projectorId) => {
    // Window features for fullscreen-like popup
    const features = [
      'menubar=no',
      'toolbar=no',
      'location=no',
      'status=no',
      'resizable=yes',
      'scrollbars=no',
      `width=${window.screen.availWidth}`,
      `height=${window.screen.availHeight}`,
      'left=0',
      'top=0',
    ].join(',');

    console.log('[Projector] Opening popup:', url);

    // CRITICAL: Safari/WebKit popup blocker workaround
    // We MUST open the window synchronously within the user gesture handler.
    // First open with about:blank to capture the gesture, then navigate.
    // This prevents popup blocking in Safari and Tauri's WebKit webview.
    const popup = window.open('about:blank', `projector-${projectorId}`, features);

    if (!popup) {
      console.error('[Projector] Popup was blocked or failed to open');
      return null;
    }

    console.log('[Projector] Popup opened successfully, navigating to:', url);

    // Now navigate to the actual URL
    // Using location.href instead of location.replace to maintain history
    popup.location.href = url;

    // Focus the popup window
    popup.focus();

    return popup;
  },

  _openTauriWindow: async (config) => {
    console.log('[Projector] _openTauriWindow starting for:', config.id);

    try {
      const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
      console.log('[Projector] WebviewWindow imported successfully');

      // Close existing projector with same ID if any
      const existing = await WebviewWindow.getByLabel(config.id);
      if (existing) {
        console.log('[Projector] Closing existing window:', config.id);
        await existing.close();
      }

      const url = buildProjectorUrl(config);
      const isFullscreen = config.displayMode === 'fullscreen';

      console.log('[Projector] Creating Tauri window:', {
        label: config.id,
        url,
        isFullscreen,
        alwaysOnTop: config.alwaysOnTop,
      });

      // Create new projector window using Tauri's WebviewWindow constructor
      // This bypasses browser popup blocking as it uses Tauri's native IPC
      const projector = new WebviewWindow(config.id, {
        url,
        fullscreen: isFullscreen,
        decorations: !isFullscreen, // No window decorations in fullscreen
        alwaysOnTop: config.alwaysOnTop,
        title: `SpiritStream Projector - ${config.type}`,
        // Windowed mode: reasonable default size
        // Fullscreen mode: these are ignored when fullscreen=true
        width: 1280,
        height: 720,
        center: !isFullscreen, // Center windowed mode
      });

      // Listen for successful window creation
      projector.once('tauri://created', () => {
        console.log('[Projector] Tauri window created successfully:', config.id);
      });

      // Listen for window creation errors
      projector.once('tauri://error', (e: unknown) => {
        console.error('[Projector] Tauri window error:', e);
        get()._cleanupProjector(config.id);
      });

      // Listen for close
      projector.once('tauri://destroyed', () => {
        console.log('[Projector] Tauri window closed:', config.id);
        get()._cleanupProjector(config.id);
      });
    } catch (err) {
      console.error('[Projector] Failed to import or create Tauri window:', err);
      throw err;
    }
  },

  _cleanupProjector: (projectorId) => {
    set((state) => {
      const newProjectors = new Map(state.projectors);
      newProjectors.delete(projectorId);
      return { projectors: newProjectors };
    });
  },

  // Legacy methods for backward compatibility
  openProjectorWindow: (profileName, sceneId) => {
    // This is called synchronously, so we use the synchronous path
    get().openProjector({
      type: 'scene',
      displayMode: 'windowed',
      targetId: sceneId,
      profileName,
      alwaysOnTop: false,
      hideCursor: false,
    });
  },

  closeProjectorWindow: async () => {
    get().closeAllProjectors();
  },
}));
