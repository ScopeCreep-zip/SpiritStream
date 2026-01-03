import { create } from 'zustand';
import { api } from '@/lib/tauri';
import i18n from '@/lib/i18n';
import type { Profile, ProfileSummary, OutputGroup, StreamTarget } from '@/types/profile';

interface ProfileState {
  // State
  profiles: ProfileSummary[];
  current: Profile | null;
  loading: boolean;
  error: string | null;

  // Encryption state
  pendingPasswordProfile: string | null;  // Profile name awaiting password
  passwordError: string | null;           // Error from failed password attempt

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
  updateStreamTarget: (groupId: string, targetId: string, updates: Partial<StreamTarget>) => Promise<void>;
  removeStreamTarget: (groupId: string, targetId: string) => Promise<void>;
}

// Helper to create a summary from a full profile
const createSummary = (profile: Profile): ProfileSummary => {
  const firstGroup = profile.outputGroups[0];
  return {
    id: profile.id,
    name: profile.name,
    resolution: firstGroup?.resolution || '1920x1080',
    bitrate: firstGroup?.videoBitrate || 6000,
    targetCount: profile.outputGroups.reduce((sum, g) => sum + g.streamTargets.length, 0),
  };
};

export const useProfileStore = create<ProfileState>((set, get) => ({
  profiles: [],
  current: null,
  loading: false,
  error: null,
  pendingPasswordProfile: null,
  passwordError: null,

  // Load all profile names from backend
  loadProfiles: async () => {
    set({ loading: true, error: null });
    try {
      const names = await api.profile.getAll();
      // Load each profile to build summaries (skip encrypted ones)
      const summaries: ProfileSummary[] = [];
      for (const name of names) {
        try {
          // Check if encrypted first
          const isEncrypted = await api.profile.isEncrypted(name);
          if (isEncrypted) {
            // Create placeholder summary for encrypted profiles
            summaries.push({
              id: name,
              name,
              resolution: i18n.t('common.encrypted'),
              bitrate: 0,
              targetCount: 0,
              isEncrypted: true,
            });
          } else {
            const profile = await api.profile.load(name);
            summaries.push({ ...createSummary(profile), isEncrypted: false });
          }
        } catch {
          console.warn(`Failed to load profile: ${name}`);
        }
      }
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

  cancelPasswordPrompt: () => set({
    pendingPasswordProfile: null,
    passwordError: null,
    loading: false
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

  // Create a new profile
  createProfile: async (name) => {
    const newProfile: Profile = {
      id: crypto.randomUUID(),
      name,
      incomingUrl: 'rtmp://localhost/live',
      outputGroups: [],
    };
    set({ current: newProfile });
    // Save to backend
    try {
      await api.profile.save(newProfile);
      const profiles = [...get().profiles, createSummary(newProfile)];
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
            g.id === groupId
              ? { ...g, streamTargets: [...g.streamTargets, target] }
              : g
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
}));
