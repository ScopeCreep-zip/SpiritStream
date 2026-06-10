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

const removeToastById = (id: string) => (state: ToastStore) => ({
  toasts: state.toasts.filter((t) => t.id !== id),
});

const appendToast = (toast: Toast) => (state: ToastStore) => ({
  toasts: [...state.toasts, toast],
});

export const useToast = create<ToastStore>((set) => ({
  toasts: [],
  addToast: (type, message) => {
    // Check if notifications are enabled (always show errors regardless)
    const { showNotifications } = useSettingsStore.getState();
    if (!showNotifications && type !== 'error') {
      return;
    }

    const id = crypto.randomUUID();
    set(appendToast({ id, type, message }));
    // Errors stay until dismissed — a 3s flash is not enough time for
    // the users this app serves (screen-reader latency, panic moments)
    // to read why their stream or safety action failed.
    if (type !== 'error') {
      setTimeout(() => set(removeToastById(id)), 3000);
    }
  },
  removeToast: (id) => set(removeToastById(id)),
}));

// Helper functions for convenience
export const toast = {
  success: (message: string) => useToast.getState().addToast('success', message),
  error: (message: string) => useToast.getState().addToast('error', message),
  info: (message: string) => useToast.getState().addToast('info', message),
};
