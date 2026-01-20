import { create } from 'zustand';
import { useSettingsStore } from '@/stores/settingsStore';

export type ToastType = 'success' | 'error' | 'info';

export interface Toast {
  id: string;
  type: ToastType;
  message: string;
}

interface ToastStore {
  toasts: Toast[];
  addToast: (type: ToastType, message: string) => void;
  removeToast: (id: string) => void;
}

export const useToast = create<ToastStore>((set) => ({
  toasts: [],
  addToast: (type, message) => {
    // Check if notifications are enabled (always show errors regardless)
    const { showNotifications } = useSettingsStore.getState();
    if (!showNotifications && type !== 'error') {
      return;
    }

    const id = crypto.randomUUID();
    set((state) => ({
      toasts: [...state.toasts, { id, type, message }],
    }));
    // Auto-remove after 3 seconds
    setTimeout(() => {
      set((state) => ({
        toasts: state.toasts.filter((t) => t.id !== id),
      }));
    }, 3000);
  },
  removeToast: (id) => {
    set((state) => ({
      toasts: state.toasts.filter((t) => t.id !== id),
    }));
  },
}));

// Helper functions for convenience
export const toast = {
  success: (message: string) => useToast.getState().addToast('success', message),
  error: (message: string) => useToast.getState().addToast('error', message),
  info: (message: string) => useToast.getState().addToast('info', message),
};
