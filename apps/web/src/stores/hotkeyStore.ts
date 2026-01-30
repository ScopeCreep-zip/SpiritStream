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
   * Reset all bindings to defaults
   */
  resetToDefaults: () => void;
}

export const useHotkeyStore = create<HotkeyState>()(
  persist(
    (set) => ({
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

      resetToDefaults: () => set({ bindings: DEFAULT_BINDINGS }),
    }),
    {
      name: 'spiritstream-hotkeys',
      version: 1,
    }
  )
);
