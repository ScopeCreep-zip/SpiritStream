import type { StateCreator } from 'zustand';
import { api } from '@/lib/client';
import { logger } from '@/lib/logger';
import type { ProfileState } from './types';

type PasswordSlice = Pick<
  ProfileState,
  | 'pendingPasswordProfile'
  | 'passwordError'
  | 'pendingUnlock'
  | 'clearPasswordError'
  | 'submitPassword'
  | 'cancelPasswordPrompt'
  | 'unlockProfile'
>;

export const createPasswordSlice: StateCreator<ProfileState, [], [], PasswordSlice> = (
  set,
  get
) => ({
  pendingPasswordProfile: null,
  passwordError: null,
  pendingUnlock: false,

  clearPasswordError: () => set({ passwordError: null }),

  submitPassword: async (password) => {
    const name = get().pendingPasswordProfile;
    const isUnlocking = get().pendingUnlock;
    if (!name) return;

    if (isUnlocking) {
      // Atomic encryption removal — one round trip. Backend loads with the
      // password and re-saves without encryption in a single operation.
      try {
        await api.profile.decrypt(name, password);
        await get().loadProfiles();
        await get().loadProfile(name);
        set({ pendingUnlock: false, pendingPasswordProfile: null, passwordError: null });
      } catch (error) {
        const message = String(error);
        if (message.toLowerCase().includes('password')) {
          set({ passwordError: 'Incorrect password', pendingUnlock: false });
        } else {
          logger.error('[ProfileStore] Failed to remove encryption:', error);
          set({ pendingUnlock: false });
          get().setError(message);
        }
      }
      return;
    }

    // Plain unlock for session: load with the password.
    await get().loadProfile(name, password);
  },

  cancelPasswordPrompt: () => {
    set({
      pendingPasswordProfile: null,
      passwordError: null,
      pendingUnlock: false,
    });
    get().setLoading(false);
  },

  unlockProfile: (name) => {
    set({
      pendingPasswordProfile: name,
      pendingUnlock: true,
      passwordError: null,
    });
  },
});
