import { create } from 'zustand';
import { api } from '@/lib/backend';
import i18n from '@/lib/i18n';
import {
  type Profile,
  type ProfileSummary,
  type OutputGroup,
  type StreamTarget,
  type Platform,
  createDefaultProfile,
} from '@/types/profile';

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

// Helper to create a summary from a full profile (using new nested structure)
const createSummary = (profile: Profile, isEncrypted: boolean = false): ProfileSummary => {
  const firstGroup = profile.outputGroups[0];

  // Build resolution string from video settings (e.g., "1080p60")
  const resolution = firstGroup ? `${firstGroup.video.height}p${firstGroup.video.fps}` : '0p0';

  // Parse bitrate from string (e.g., "6000k" -> 6000)
  const bitrateStr = firstGroup?.video.bitrate || '0k';
  const bitrate = parseInt(bitrateStr.replace(/[^\d]/g, ''), 10) || 0;

  // Count all stream targets across all groups
  const targetCount = profile.outputGroups.reduce((sum, g) => sum + g.streamTargets.length, 0);

  // Collect unique services from all targets
  const servicesSet = new Set<Platform>();
  for (const group of profile.outputGroups) {
    for (const target of group.streamTargets) {
      servicesSet.add(target.service);
    }
  }
  const services = Array.from(servicesSet);

  return {
    id: profile.id,
    name: profile.name,
    resolution,
    bitrate,
    targetCount,
    services,
    isEncrypted,
  };
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

      const profile = await api.profile.load(name, password);
      console.log('[ProfileStore] Profile loaded from backend:', {
        profileId: profile.id,
        profileName: profile.name,
        chatConfigsCount: profile.chatConfigs?.length || 0,
        chatPlatforms: profile.chatConfigs?.map((c) => c.platform) || [],
      });
      set({
        current: profile,
        loading: false,
        pendingPasswordProfile: null,
        passwordError: null,
      });

      // Update summary with actual data if it was encrypted
      if (isEncrypted) {
        const summaries = get().profiles.map((s) =>
          s.name === name ? { ...createSummary(profile), isEncrypted: true } : s
        );
        set({ profiles: summaries });
      }

      // Save as last used profile
      try {
        const settings = await api.settings.get();
        if (settings.lastProfile !== name) {
          await api.settings.save({ ...settings, lastProfile: name });
        }
      } catch (settingsError) {
        console.warn('[ProfileStore] Failed to save last profile:', settingsError);
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

    // Load the profile with the password
    await get().loadProfile(name, password);

    // If we were unlocking and profile loaded successfully, save without password to remove encryption
    if (isUnlocking && get().current && !get().passwordError) {
      try {
        const profile = get().current!;
        await api.profile.save(profile); // No password = unencrypted
        // Reload profiles to update encryption status
        await get().loadProfiles();
        set({ pendingUnlock: false });
      } catch (error) {
        console.error('[ProfileStore] Failed to remove encryption:', error);
        set({ error: String(error), pendingUnlock: false });
      }
    }
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

    console.log('[ProfileStore] saveProfile called:', {
      profileId: current.id,
      profileName: current.name,
      chatConfigsCount: current.chatConfigs?.length || 0,
      hasPassword: !!password,
    });

    // Don't set loading: true - this causes UI to flash "Loading..."
    // The caller should have already updated the state optimistically
    set({ error: null });
    try {
      await api.profile.save(current, password);
      console.log('[ProfileStore] saveProfile completed (backend save successful)');
      // Update the summary in the list
      const summaries = get().profiles.map((s) =>
        s.name === current.name ? createSummary(current) : s
      );
      // Add if not exists
      if (!summaries.find((s) => s.name === current.name)) {
        summaries.push(createSummary(current));
      }
      set({ profiles: summaries });
    } catch (error) {
      console.error('[ProfileStore] saveProfile failed:', error);
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
      const profiles = [...get().profiles, createSummary(newProfile, false)];
      set({ profiles });
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
      const profile = await api.profile.load(name);
      const newProfile: Profile = {
        ...profile,
        id: crypto.randomUUID(),
        name: `${profile.name} ${i18n.t('common.copySuffix')}`,
      };
      await api.profile.save(newProfile);
      const profiles = [...get().profiles, createSummary(newProfile)];
      set({ profiles });
    } catch (error) {
      set({ error: String(error) });
    }
  },

  updateProfile: async (updates) => {
    const current = get().current;
    if (current) {
      const updatedProfile = { ...current, ...updates };
      console.log('[ProfileStore] updateProfile called:', {
        profileId: current.id,
        profileName: current.name,
        updateKeys: Object.keys(updates),
        chatConfigsCount: (updates.chatConfigs as any)?.length,
        updatedChatConfigsCount: updatedProfile.chatConfigs?.length,
      });
      set({ current: updatedProfile });
      await get().saveProfile();
      console.log('[ProfileStore] updateProfile completed (saveProfile called)');
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
        console.warn('Cannot delete the default passthrough output group');
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
