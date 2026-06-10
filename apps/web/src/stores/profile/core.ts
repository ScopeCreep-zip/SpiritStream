import type { StateCreator } from 'zustand';
import { api } from '@/lib/client';
import { logger } from '@/lib/logger';
import { toast } from '@/hooks/useToast';
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
  | 'loadProfiles'
  | 'loadProfile'
  | 'saveProfile'
  | 'deleteProfile'
  | 'createProfile'
  | 'reorderProfiles'
  | 'setLoading'
  | 'selectProfile'
  | 'duplicateProfile'
  | 'updateProfile'
  | 'updateProfileSettings'
>;

export const createCoreSlice: StateCreator<ProfileState, [], [], CoreSlice> = (set, get) => ({
  profiles: [],
  current: null,
  loading: false,

  loadProfiles: async () => {
    const isInitialLoad = get().profiles.length === 0;
    if (isInitialLoad) {
      get().setLoading(true);
    }
    try {
      const summaries = await api.profile.getSummaries();
      set({ profiles: summaries });
      get().setLoading(false);
    } catch (error) {
      logger.error('[ProfileStore] loadProfiles failed:', error);
      toast.error(
        i18n.t('errors.loadProfilesFailed', {
          defaultValue: 'Failed to load profiles: {{error}}',
          error: error instanceof Error ? error.message : String(error),
        })
      );
      get().setLoading(false);
    }
  },

  loadProfile: async (name, password) => {
    const currentName = get().current?.name;
    const isRefresh = currentName === name;

    if (!isRefresh) {
      get().setLoading(true);
    }
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
      // Branch on the structured `kind` the api-client attaches (from
      // CoreError's serde tag), not message substrings.
      const kind = (error as Error & { kind?: string }).kind;
      if (password && (kind === 'password_incorrect' || kind === 'password_required')) {
        set({ passwordError: i18n.t('login.incorrectPassword', 'Incorrect password') });
        get().setLoading(false);
      } else {
        logger.error('[ProfileStore] loadProfile failed:', error);
        set({ pendingPasswordProfile: null });
        toast.error(
          i18n.t('errors.loadProfileFailed', {
            defaultValue: 'Failed to open profile {{name}}: {{error}}',
            name,
            error: error instanceof Error ? error.message : String(error),
          })
        );
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
    try {
      await api.profile.save(current, password);
      logger.debug('[ProfileStore] saveProfile completed (backend save successful)');
      await get().loadProfiles();
    } catch (error) {
      logger.error('[ProfileStore] saveProfile failed:', error);
      // Rethrow so callers can surface the failure. Swallowing here
      // meant every profile mutation reported success (toast, closed
      // modal) while the backend had rejected the save — the in-memory
      // state silently diverged from disk until the next reload.
      throw error;
    }
  },

  deleteProfile: async (name) => {
    get().setLoading(true);
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
      get().setLoading(false);
      // Rethrow — FileMenu's catch (and its "Deleted profile" success
      // toast guard) was dead while this swallowed.
      throw error;
    }
  },

  createProfile: async (name) => {
    const newProfile = createDefaultProfile(name);
    set({ current: newProfile });
    await api.profile.save(newProfile);
    await get().loadProfiles();
  },

  setLoading: (loading) => set({ loading }),

  selectProfile: async (name) => {
    await get().loadProfile(name);
  },

  duplicateProfile: async (name) => {
    const profile = await api.profile.load(name, undefined, false);
    const newProfile: Profile = {
      ...profile,
      id: crypto.randomUUID(),
      name: `${profile.name} ${i18n.t('common.copySuffix')}`,
    };
    await api.profile.save(newProfile);
    await get().loadProfiles();
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
      toast.error(
        i18n.t('errors.reorderProfilesFailed', {
          defaultValue: 'Failed to reorder profiles: {{error}}',
          error: err instanceof Error ? err.message : String(err),
        })
      );
    }
  },
});
