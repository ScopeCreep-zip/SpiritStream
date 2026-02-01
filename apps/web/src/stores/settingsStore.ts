import { create } from 'zustand';

/**
 * Global settings store for app-wide settings that need to be accessed
 * from multiple places (e.g., toast notifications respecting showNotifications).
 */
interface SettingsStore {
  showNotifications: boolean;
  setShowNotifications: (value: boolean) => void;
}

export const useSettingsStore = create<SettingsStore>((set) => ({
  showNotifications: true, // Default to true
  setShowNotifications: (value) => set({ showNotifications: value }),
}));
