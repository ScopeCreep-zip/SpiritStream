import { create } from 'zustand';
import { api } from '@/lib/tauri';
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

  // Async actions (Tauri integration)
  loadProfiles: () => Promise<void>;
  loadProfile: (name: string, password?: string) => Promise<void>;
  saveProfile: (password?: string) => Promise<void>;
  deleteProfile: (name: string) => Promise<void>;
  createProfile: (name: string) => Promise<void>;
  isProfileEncrypted: (name: string) => Promise<boolean>;

  // Password modal actions
  setPendingPasswordProfile: (name: string | null) => void;
  clearPasswordError: () => void;
  submitPassword: (password: string) => Promise<void>;
  cancelPasswordPrompt: () => void;

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

  // Load all profile summaries from backend (uses efficient getSummaries endpoint)
  loadProfiles: async () => {
    set({ loading: true, error: null });
    try {
      // Use the new getSummaries endpoint that returns all summaries with services
      const summaries = await api.profile.getSummaries();
      set({ profiles: summaries, loading: false });
    } catch (error) {
      set({ error: String(error), loading: false });
    }
  },

  // Load a specific profile by name
  loadProfile: async (name, password) => {
    set({ loading: true, error: null, passwordError: null });
    try {
      // Check if profile is encrypted and no password provided
      const isEncrypted = await api.profile.isEncrypted(name);
      if (isEncrypted && !password) {
        // Trigger password modal
        set({ loading: false, pendingPasswordProfile: name });
        return;
      }

      const profile = await api.profile.load(name, password);
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
    if (!name) return;
    await get().loadProfile(name, password);
  },

  cancelPasswordPrompt: () =>
    set({
      pendingPasswordProfile: null,
      passwordError: null,
      loading: false,
    }),

  // Save the current profile to backend
  saveProfile: async (password) => {
    const current = get().current;
    if (!current) return;

    set({ loading: true, error: null });
    try {
      await api.profile.save(current, password);
      // Update the summary in the list
      const summaries = get().profiles.map((s) =>
        s.name === current.name ? createSummary(current) : s
      );
      // Add if not exists
      if (!summaries.find((s) => s.name === current.name)) {
        summaries.push(createSummary(current));
      }
      set({ profiles: summaries, loading: false });
    } catch (error) {
      set({ error: String(error), loading: false });
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
      set({ current: { ...current, ...updates } });
      await get().saveProfile();
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
