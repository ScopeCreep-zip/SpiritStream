import { create } from 'zustand';
import { api } from '@/lib/client';
import { events } from '@spiritstream/api-client';
import { logger } from '@/lib/logger';
import i18n from '@/lib/i18n';
import type { Profile, ProfileSummary, ProfileSettings, OutputGroup, StreamTarget, ProfileActivatedEvent } from '@spiritstream/types';
import { createDefaultProfile } from '@/lib/profile-helpers';
import { useThemeStore } from '@/stores/themeStore';
import { useLanguageStore, type Language } from '@/stores/languageStore';
import { useSettingsStore } from '@/stores/settingsStore';
import { toast } from '@/hooks/useToast';

interface ProfileState {
  // State
  profiles: ProfileSummary[];
  current: Profile | null;
  loading: boolean;
  error: string | null;

  // Encryption state
  pendingPasswordProfile: string | null; // Profile name awaiting password
  passwordError: string | null; // Error from failed password attempt
  pendingUnlock: boolean; // True if we're unlocking (removing password) rather than just loading

  // Async actions (Tauri integration)
  loadProfiles: () => Promise<void>;
  loadProfile: (name: string, password?: string) => Promise<void>;
  saveProfile: (password?: string) => Promise<void>;
  deleteProfile: (name: string) => Promise<void>;
  createProfile: (name: string) => Promise<void>;
  isProfileEncrypted: (name: string) => Promise<boolean>;
  reorderProfiles: (fromIndex: number, toIndex: number) => Promise<void>;

  // Password modal actions
  setPendingPasswordProfile: (name: string | null) => void;
  clearPasswordError: () => void;
  submitPassword: (password: string) => Promise<void>;
  cancelPasswordPrompt: () => void;
  unlockProfile: (name: string) => void; // Start unlock flow (prompts for password, then removes encryption)

  // Sync actions
  setProfiles: (profiles: ProfileSummary[]) => void;
  setCurrentProfile: (profile: Profile | null) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  selectProfile: (name: string) => Promise<void>;
  duplicateProfile: (name: string) => Promise<void>;

  // Profile mutations (local state updates + auto-save)
  updateProfile: (updates: Partial<Profile>) => Promise<void>;

  // Profile settings mutations
  updateProfileSettings: (updates: Partial<ProfileSettings>) => Promise<void>;

  // Output group mutations (auto-save)
  addOutputGroup: (group: OutputGroup) => Promise<void>;
  updateOutputGroup: (groupId: string, updates: Partial<OutputGroup>) => Promise<void>;
  removeOutputGroup: (groupId: string) => Promise<void>;

  // Stream target mutations (auto-save)
  addStreamTarget: (groupId: string, target: StreamTarget) => Promise<void>;
  updateStreamTarget: (
    groupId: string,
    targetId: string,
    updates: Partial<StreamTarget>
  ) => Promise<void>;
  removeStreamTarget: (groupId: string, targetId: string) => Promise<void>;
  moveStreamTarget: (fromGroupId: string, toGroupId: string, targetId: string) => Promise<void>;
}

/**
 * Apply UI-only profile settings (theme, language, notifications)
 * Called when settings are updated on the current profile
 */
const applyUiSettings = (settings: ProfileSettings) => {
  // Apply theme
  if (settings.themeId) {
    useThemeStore.getState().setTheme(settings.themeId);
  }

  // Apply language
  if (settings.language) {
    useLanguageStore.getState().setLanguage(settings.language as Language);
  }

  // Apply notification settings
  useSettingsStore.getState().setShowNotifications(settings.showNotifications);
};

/**
 * Apply the UI-only consequences of a profile activation (theme, language,
 * notification preference). The backend's `ProfileService::activate()` owns
 * OBS disconnect/reconfigure/auto-connect — the frontend never orchestrates
 * those, it just receives the resulting obs:// events through the
 * `useObsStore` subscription. The frontend just renders.
 */
const applyProfileSettings = (settings: ProfileSettings) => {
  applyUiSettings(settings);
};

export const useProfileStore = create<ProfileState>((set, get) => ({
  profiles: [],
  current: null,
  loading: false,
  error: null,
  pendingPasswordProfile: null,
  passwordError: null,
  pendingUnlock: false,

  // Load all profile summaries from backend (uses efficient getSummaries endpoint)
  // Only shows loading on first load (when profiles array is empty)
  // Background refreshes (e.g., from remote sync) don't show loading
  loadProfiles: async () => {
    const isInitialLoad = get().profiles.length === 0;
    if (isInitialLoad) {
      set({ loading: true, error: null });
    } else {
      set({ error: null });
    }
    try {
      // Use the new getSummaries endpoint that returns all summaries with services
      const summaries = await api.profile.getSummaries();
      set({ profiles: summaries, loading: false });
    } catch (error) {
      set({ error: String(error), loading: false });
    }
  },

  // Load a specific profile by name
  // Only shows loading if loading a different profile (not a background refresh)
  loadProfile: async (name, password) => {
    const currentName = get().current?.name;
    const isRefresh = currentName === name;

    // Only show loading spinner if we're switching profiles, not refreshing current
    if (!isRefresh) {
      set({ loading: true, error: null, passwordError: null });
    } else {
      set({ error: null, passwordError: null });
    }
    try {
      // Check if profile is encrypted and no password provided
      const isEncrypted = await api.profile.isEncrypted(name);
      if (isEncrypted && !password) {
        // Trigger password modal
        set({ loading: false, pendingPasswordProfile: name });
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
        loading: false,
        pendingPasswordProfile: null,
        passwordError: null,
      });
      // Apply the activated profile's UI-only settings (theme, language,
      // notifications) synchronously here as well. The backend ALSO emits a
      // `profile_activated` event that drives the same cascade — both paths
      // converge, but the synchronous call closes the boot-time race where
      // the event-bus listener hasn't finished its WS handshake yet.
      applyProfileSettings(profile.settings);

      // Refresh the summary list from the backend so the parsed numeric
      // bitrate / resolution / target count come from `Profile::to_summary()`,
      // not from a duplicated client-side `createSummary`.
      if (isEncrypted) {
        await get().loadProfiles();
      }

      // Save as last used profile
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
      // If password was wrong, set password error
      if (password && errorMsg.includes('decrypt')) {
        set({ passwordError: 'Incorrect password', loading: false });
      } else {
        set({ error: errorMsg, loading: false, pendingPasswordProfile: null });
      }
    }
  },

  // Check if a profile is encrypted
  isProfileEncrypted: async (name) => {
    return await api.profile.isEncrypted(name);
  },

  // Password modal actions
  setPendingPasswordProfile: (name) => set({ pendingPasswordProfile: name, passwordError: null }),
  clearPasswordError: () => set({ passwordError: null }),

  submitPassword: async (password) => {
    const name = get().pendingPasswordProfile;
    const isUnlocking = get().pendingUnlock;
    if (!name) return;

    if (isUnlocking) {
      // Atomic encryption removal — one round trip. Backend loads with the
      // password and re-saves without encryption in a single operation.
      // Replaces the legacy `loadProfile(password) + saveProfile()` flow.
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
          set({ error: message, pendingUnlock: false });
        }
      }
      return;
    }

    // Plain unlock for session: load with the password.
    await get().loadProfile(name, password);
  },

  cancelPasswordPrompt: () =>
    set({
      pendingPasswordProfile: null,
      passwordError: null,
      pendingUnlock: false,
      loading: false,
    }),

  // Start unlock flow - prompts for password, then removes encryption
  unlockProfile: (name) => {
    set({
      pendingPasswordProfile: name,
      pendingUnlock: true,
      passwordError: null,
    });
  },

  // Save the current profile to backend
  // Uses optimistic UI - state is already updated before this is called,
  // so we don't set loading to avoid UI flicker
  saveProfile: async (password) => {
    const current = get().current;
    if (!current) return;

    logger.debug('[ProfileStore] saveProfile called:', {
      profileId: current.id,
      profileName: current.name,
      hasPassword: !!password,
    });

    // Don't set loading: true - this causes UI to flash "Loading..."
    // The caller should have already updated the state optimistically
    set({ error: null });
    try {
      await api.profile.save(current, password);
      logger.debug('[ProfileStore] saveProfile completed (backend save successful)');
      // Refresh the summary list from the backend — `Profile::to_summary()`
      // is the single source of truth for parsed numeric fields.
      await get().loadProfiles();
    } catch (error) {
      logger.error('[ProfileStore] saveProfile failed:', error);
      set({ error: String(error) });
    }
  },

  // Delete a profile by name
  deleteProfile: async (name) => {
    set({ loading: true, error: null });
    try {
      await api.profile.delete(name);
      const profiles = get().profiles.filter((p) => p.name !== name);
      const current = get().current;
      set({
        profiles,
        current: current?.name === name ? null : current,
        loading: false,
      });
      // Clear `lastProfile` if it pointed at the deleted name —
      // otherwise the next boot tries to activate a missing profile and the
      // user lands on the no-profile UI without explanation. The Rust shape
      // is `Option<String>` → TS `string | null`; null is the idiomatic
      // "no last profile" signal (empty string round-trips as `Some("")`).
      try {
        const settings = await api.settings.get();
        if (settings.lastProfile === name) {
          await api.settings.save({ ...settings, lastProfile: null });
        }
      } catch (clearError) {
        logger.warn('[ProfileStore] Failed to clear lastProfile after delete:', clearError);
      }
    } catch (error) {
      set({ error: String(error), loading: false });
    }
  },

  // Create a new profile using the default template (new nested structure)
  createProfile: async (name) => {
    const newProfile = createDefaultProfile(name);
    set({ current: newProfile });
    // Save to backend
    try {
      await api.profile.save(newProfile);
      await get().loadProfiles();
    } catch (error) {
      set({ error: String(error) });
    }
  },

  setProfiles: (profiles) => set({ profiles }),
  setCurrentProfile: (profile) => set({ current: profile }),
  setLoading: (loading) => set({ loading }),
  setError: (error) => set({ error }),

  // Select and load a profile by name
  selectProfile: async (name) => {
    await get().loadProfile(name);
  },

  // Duplicate a profile
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
      set({ error: String(error) });
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
      // Apply the UI-only settings (don't disconnect OBS for settings changes)
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

    // optimistic UI update
    set({ profiles: next });

    try {
      await api.profile.setProfileOrder(next.map(p => p.name));
    } catch (err) {
      // revert on failure + surface error
      set({ profiles, error: String(err) });
    }
  },

  addOutputGroup: async (group) => {
    const current = get().current;
    if (current) {
      set({
        current: {
          ...current,
          outputGroups: [...current.outputGroups, group],
        },
      });
      await get().saveProfile();
    }
  },

  updateOutputGroup: async (groupId, updates) => {
    const current = get().current;
    if (current) {
      set({
        current: {
          ...current,
          outputGroups: current.outputGroups.map((g) =>
            g.id === groupId ? { ...g, ...updates } : g
          ),
        },
      });
      await get().saveProfile();
    }
  },

  removeOutputGroup: async (groupId) => {
    const current = get().current;
    if (current) {
      // Prevent deletion of the default passthrough group
      const groupToDelete = current.outputGroups.find((g) => g.id === groupId);
      if (groupToDelete?.isDefault) {
        logger.warn('Cannot delete the default passthrough output group');
        return;
      }

      set({
        current: {
          ...current,
          outputGroups: current.outputGroups.filter((g) => g.id !== groupId),
        },
      });
      await get().saveProfile();
    }
  },

  addStreamTarget: async (groupId, target) => {
    const current = get().current;
    if (current) {
      set({
        current: {
          ...current,
          outputGroups: current.outputGroups.map((g) =>
            g.id === groupId ? { ...g, streamTargets: [...g.streamTargets, target] } : g
          ),
        },
      });
      await get().saveProfile();
    }
  },

  updateStreamTarget: async (groupId, targetId, updates) => {
    const current = get().current;
    if (current) {
      set({
        current: {
          ...current,
          outputGroups: current.outputGroups.map((g) =>
            g.id === groupId
              ? {
                  ...g,
                  streamTargets: g.streamTargets.map((t) =>
                    t.id === targetId ? { ...t, ...updates } : t
                  ),
                }
              : g
          ),
        },
      });
      await get().saveProfile();
    }
  },

  removeStreamTarget: async (groupId, targetId) => {
    const current = get().current;
    if (current) {
      set({
        current: {
          ...current,
          outputGroups: current.outputGroups.map((g) =>
            g.id === groupId
              ? { ...g, streamTargets: g.streamTargets.filter((t) => t.id !== targetId) }
              : g
          ),
        },
      });
      await get().saveProfile();
    }
  },

  moveStreamTarget: async (fromGroupId, toGroupId, targetId) => {
    const current = get().current;
    if (!current || fromGroupId === toGroupId) return;

    // Find the target in the source group
    const sourceGroup = current.outputGroups.find((g: OutputGroup) => g.id === fromGroupId);
    const target = sourceGroup?.streamTargets.find((t: StreamTarget) => t.id === targetId);
    if (!target) return;

    // Remove from source group and add to destination group
    set({
      current: {
        ...current,
        outputGroups: current.outputGroups.map((g: OutputGroup) => {
          if (g.id === fromGroupId) {
            return {
              ...g,
              streamTargets: g.streamTargets.filter((t: StreamTarget) => t.id !== targetId),
            };
          }
          if (g.id === toGroupId) {
            return { ...g, streamTargets: [...g.streamTargets, target] };
          }
          return g;
        }),
      },
    });
    await get().saveProfile();
  },
}));

/**
 * Backend → frontend bridge: the server emits a single `profile_activated`
 * event with the consolidated settings (theme, language, OBS, etc.) whenever
 * `ProfileService::activate()` runs. The payload type is generated by ts-rs
 * from `crates/core/src/services/profile_manager.rs` so the wire shape stays
 * in sync with Rust without a hand-rolled mirror.
 *
 * Subscribed from `AppContent` (after the server-readiness gate passes) —
 * NOT at module-import time. Subscribing at module-import opened the
 * WebSocket before the server was listening, producing the
 * "WebSocket connection failed" cascade in the boot console.
 */
export async function subscribeProfileActivated(): Promise<() => void> {
  return events.on<ProfileActivatedEvent>('profile_activated', (payload) => {
    const settings = useProfileStore.getState().current?.settings;
    if (!settings) return;
    applyProfileSettings({
      ...settings,
      themeId: payload.themeId,
      language: payload.language,
      showNotifications: payload.showNotifications,
    });
  });
}

/**
 * Backend → frontend bridge: the server emits `oauth_token_expired` whenever
 * a proactive refresh inside `refresh_expiring_oauth_tokens` fails — typically
 * because the refresh token itself was revoked or expired. The frontend just
 * renders; the recovery action (re-running the OAuth flow) happens when the
 * user clicks reconnect.
 *
 * Subscribed from `AppContent` (after the server-readiness gate passes) —
 * same rationale as `subscribeProfileActivated`.
 */
interface OAuthTokenExpiredPayload {
  provider: string;
}

export async function subscribeOAuthTokenExpired(): Promise<() => void> {
  return events.on<OAuthTokenExpiredPayload>('oauth_token_expired', (payload) => {
    logger.warn(
      `[ProfileStore] OAuth token expired for ${payload.provider}; re-auth required.`,
    );
    const label = payload.provider.charAt(0).toUpperCase() + payload.provider.slice(1);
    toast.info(`${label} sign-in expired — reconnect to send chat or stream.`);
  });
}
