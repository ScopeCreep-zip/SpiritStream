import type { StateCreator } from 'zustand';
import { api } from '@/lib/client';
import { logger } from '@/lib/logger';
import i18n from '@/lib/i18n';
import type { Profile } from '@spiritstream/types';
import { createDefaultProfile } from '@/lib/profile-helpers';
import { applyProfileSettings, applyUiSettings } from './applySettings';
import type { ProfileState } from './types';

type CoreSlice = Pick<
  ProfileState,
  | 'profiles'
  | 'current'
  | 'loading'
  | 'error'
  | 'loadProfiles'
  | 'loadProfile'
  | 'saveProfile'
  | 'deleteProfile'
  | 'createProfile'
  | 'reorderProfiles'
  | 'setLoading'
  | 'setError'
  | 'selectProfile'
  | 'duplicateProfile'
  | 'updateProfile'
  | 'updateProfileSettings'
>;

export const createCoreSlice: StateCreator<ProfileState, [], [], CoreSlice> = (set, get) => ({
  profiles: [],
  current: null,
  loading: false,
  error: null,

  loadProfiles: async () => {
    const isInitialLoad = get().profiles.length === 0;
    if (isInitialLoad) {
      get().setLoading(true);
    }
    get().setError(null);
    try {
      const summaries = await api.profile.getSummaries();
      set({ profiles: summaries });
      get().setLoading(false);
    } catch (error) {
      get().setError(String(error));
      get().setLoading(false);
    }
  },

  loadProfile: async (name, password) => {
    const currentName = get().current?.name;
    const isRefresh = currentName === name;

    if (!isRefresh) {
      get().setLoading(true);
    }
    get().setError(null);
    set({ passwordError: null });
    try {
      const isEncrypted = await api.profile.isEncrypted(name);
      if (isEncrypted && !password) {
        set({ pendingPasswordProfile: name });
        get().setLoading(false);
        return;
      }

      // Use the typed `/activate` endpoint: backend loads, sets active
      // session state, and emits a single `profile_activated` event whose
      // consolidated payload the UI stores listen for.
      const profile = await api.profile.activate(name, password);
      logger.debug('[ProfileStore] Profile activated via backend:', {
        profileId: profile.id,
        profileName: profile.name,
      });
      set({
        current: profile,
        pendingPasswordProfile: null,
        passwordError: null,
      });
      get().setLoading(false);
      // The backend also emits a `profile_activated` event that drives the
      // same cascade — both paths converge, but the synchronous call closes
      // the boot-time race where the event-bus listener hasn't finished its
      // WS handshake yet.
      applyProfileSettings(profile.settings);

      if (isEncrypted) {
        await get().loadProfiles();
      }

      try {
        const settings = await api.settings.get();
        if (settings.lastProfile !== name) {
          await api.settings.save({ ...settings, lastProfile: name });
        }
      } catch (settingsError) {
        logger.warn('[ProfileStore] Failed to save last profile:', settingsError);
      }
    } catch (error) {
      const errorMsg = String(error);
      if (password && errorMsg.includes('decrypt')) {
        set({ passwordError: 'Incorrect password' });
        get().setLoading(false);
      } else {
        set({ pendingPasswordProfile: null });
        get().setError(errorMsg);
        get().setLoading(false);
      }
    }
  },

  saveProfile: async (password) => {
    const current = get().current;
    if (!current) return;

    logger.debug('[ProfileStore] saveProfile called:', {
      profileId: current.id,
      profileName: current.name,
      hasPassword: !!password,
    });

    // Don't set loading: true — this causes the UI to flash "Loading...".
    // The caller should have already updated the state optimistically.
    get().setError(null);
    try {
      await api.profile.save(current, password);
      logger.debug('[ProfileStore] saveProfile completed (backend save successful)');
      await get().loadProfiles();
    } catch (error) {
      logger.error('[ProfileStore] saveProfile failed:', error);
      get().setError(String(error));
    }
  },

  deleteProfile: async (name) => {
    get().setLoading(true);
    get().setError(null);
    try {
      await api.profile.delete(name);
      const profiles = get().profiles.filter((p) => p.name !== name);
      const current = get().current;
      set({
        profiles,
        current: current?.name === name ? null : current,
      });
      get().setLoading(false);
      // Clear `lastProfile` if it pointed at the deleted name — otherwise the
      // next boot tries to activate a missing profile and the user lands on
      // the no-profile UI without explanation. Null is the idiomatic "no
      // last profile" signal (empty string round-trips as `Some("")`).
      try {
        const settings = await api.settings.get();
        if (settings.lastProfile === name) {
          await api.settings.save({ ...settings, lastProfile: null });
        }
      } catch (clearError) {
        logger.warn('[ProfileStore] Failed to clear lastProfile after delete:', clearError);
      }
    } catch (error) {
      get().setError(String(error));
      get().setLoading(false);
    }
  },

  createProfile: async (name) => {
    const newProfile = createDefaultProfile(name);
    set({ current: newProfile });
    try {
      await api.profile.save(newProfile);
      await get().loadProfiles();
    } catch (error) {
      get().setError(String(error));
    }
  },

  setLoading: (loading) => set({ loading }),
  setError: (error) => set({ error }),

  selectProfile: async (name) => {
    await get().loadProfile(name);
  },

  duplicateProfile: async (name) => {
    try {
      const profile = await api.profile.load(name, undefined, false);
      const newProfile: Profile = {
        ...profile,
        id: crypto.randomUUID(),
        name: `${profile.name} ${i18n.t('common.copySuffix')}`,
      };
      await api.profile.save(newProfile);
      await get().loadProfiles();
    } catch (error) {
      get().setError(String(error));
    }
  },

  updateProfile: async (updates) => {
    const current = get().current;
    if (current) {
      const updatedProfile = { ...current, ...updates };
      logger.debug('[ProfileStore] updateProfile called:', {
        profileId: current.id,
        profileName: current.name,
        updateKeys: Object.keys(updates),
      });
      set({ current: updatedProfile });
      await get().saveProfile();
      logger.debug('[ProfileStore] updateProfile completed (saveProfile called)');
    }
  },

  updateProfileSettings: async (updates) => {
    const current = get().current;
    if (current && current.settings) {
      const newSettings = { ...current.settings, ...updates };
      set({
        current: {
          ...current,
          settings: newSettings,
        },
      });
      // Apply UI-only settings without triggering OBS reconfigure.
      applyUiSettings(newSettings);
      await get().saveProfile();
    }
  },

  reorderProfiles: async (fromIndex, toIndex) => {
    const { profiles } = get();
    if (fromIndex === toIndex) return;
    if (fromIndex < 0 || toIndex < 0) return;
    if (fromIndex >= profiles.length || toIndex >= profiles.length) return;

    const next = profiles.slice();
    const [moved] = next.splice(fromIndex, 1);
    next.splice(toIndex, 0, moved);

    set({ profiles: next });

    try {
      await api.profile.setProfileOrder(next.map((p) => p.name));
    } catch (err) {
      // Revert on failure + surface error.
      set({ profiles });
      get().setError(String(err));
    }
  },
});
