/**
 * Hotkey Store
 * Manages global keyboard shortcut bindings
 */
import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { HotkeyBinding } from '@/types/hotkeys';
import { DEFAULT_BINDINGS } from '@/types/hotkeys';

interface HotkeyState {
  /** Whether hotkeys are globally enabled */
  enabled: boolean;
  /** Current hotkey bindings */
  bindings: HotkeyBinding[];

  /**
   * Enable or disable all hotkeys
   */
  setEnabled: (enabled: boolean) => void;

  /**
   * Toggle hotkeys on/off
   */
  toggleEnabled: () => void;

  /**
   * Update a specific binding
   */
  updateBinding: (id: string, updates: Partial<HotkeyBinding>) => void;

  /**
   * Enable or disable a specific binding
   */
  setBindingEnabled: (id: string, enabled: boolean) => void;

  /**
   * Add a new binding (for layer hotkeys)
   */
  addBinding: (binding: HotkeyBinding) => void;

  /**
   * Remove a binding by ID
   */
  removeBinding: (id: string) => void;

  /**
   * Get binding for a specific layer
   */
  getLayerBinding: (layerId: string, sceneId: string) => HotkeyBinding | undefined;

  /**
   * Set or update layer visibility hotkey
   */
  setLayerHotkey: (
    layerId: string,
    sceneId: string,
    key: string,
    displayKey: string,
    modifiers: HotkeyBinding['modifiers']
  ) => void;

  /**
   * Remove layer visibility hotkey
   */
  removeLayerHotkey: (layerId: string, sceneId: string) => void;

  /**
   * Reset all bindings to defaults
   */
  resetToDefaults: () => void;
}

export const useHotkeyStore = create<HotkeyState>()(
  persist(
    (set, get) => ({
      enabled: true,
      bindings: DEFAULT_BINDINGS,

      setEnabled: (enabled) => set({ enabled }),

      toggleEnabled: () => set((state) => ({ enabled: !state.enabled })),

      updateBinding: (id, updates) =>
        set((state) => ({
          bindings: state.bindings.map((b) =>
            b.id === id ? { ...b, ...updates } : b
          ),
        })),

      setBindingEnabled: (id, enabled) =>
        set((state) => ({
          bindings: state.bindings.map((b) =>
            b.id === id ? { ...b, enabled } : b
          ),
        })),

      addBinding: (binding) =>
        set((state) => ({
          bindings: [...state.bindings, binding],
        })),

      removeBinding: (id) =>
        set((state) => ({
          bindings: state.bindings.filter((b) => b.id !== id),
        })),

      getLayerBinding: (layerId, sceneId) => {
        return get().bindings.find(
          (b) =>
            b.action === 'toggleLayerVisibility' &&
            b.layerId === layerId &&
            b.sceneId === sceneId
        );
      },

      setLayerHotkey: (layerId, sceneId, key, displayKey, modifiers) =>
        set((state) => {
          // Remove existing binding for this layer if any
          const filtered = state.bindings.filter(
            (b) =>
              !(
                b.action === 'toggleLayerVisibility' &&
                b.layerId === layerId &&
                b.sceneId === sceneId
              )
          );

          // Add new binding
          const newBinding: HotkeyBinding = {
            id: `layer-${layerId}`,
            action: 'toggleLayerVisibility',
            key,
            displayKey,
            modifiers,
            enabled: true,
            layerId,
            sceneId,
          };

          return { bindings: [...filtered, newBinding] };
        }),

      removeLayerHotkey: (layerId, sceneId) =>
        set((state) => ({
          bindings: state.bindings.filter(
            (b) =>
              !(
                b.action === 'toggleLayerVisibility' &&
                b.layerId === layerId &&
                b.sceneId === sceneId
              )
          ),
        })),

      resetToDefaults: () => set({ bindings: DEFAULT_BINDINGS }),
    }),
    {
      name: 'spiritstream-hotkeys',
      version: 1,
    }
  )
);
