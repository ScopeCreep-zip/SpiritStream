import type { StateCreator } from 'zustand';
import { api } from '@/lib/client';
import { logger } from '@/lib/logger';
import { toast } from '@/hooks/useToast';
import i18n from '@/lib/i18n';
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
        const kind = (error as Error & { kind?: string }).kind;
        if (kind === 'password_incorrect' || kind === 'password_required') {
          set({ passwordError: 'Incorrect password', pendingUnlock: false });
        } else {
          logger.error('[ProfileStore] Failed to remove encryption:', error);
          set({ pendingUnlock: false });
          toast.error(
            i18n.t('errors.removeEncryptionFailed', {
              defaultValue: 'Failed to remove encryption: {{error}}',
              error: error instanceof Error ? error.message : String(error),
            })
          );
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
