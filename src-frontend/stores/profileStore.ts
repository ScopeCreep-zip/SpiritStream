import { create } from 'zustand';
import type { Profile, ProfileSummary, OutputGroup, StreamTarget } from '@/types/profile';

interface ProfileState {
  // State
  profiles: ProfileSummary[];
  current: Profile | null;
  loading: boolean;
  error: string | null;

  // Actions
  setProfiles: (profiles: ProfileSummary[]) => void;
  setCurrentProfile: (profile: Profile | null) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  selectProfile: (id: string) => void;
  duplicateProfile: (id: string) => void;

  // Profile mutations
  updateProfile: (updates: Partial<Profile>) => void;

  // Output group mutations
  addOutputGroup: (group: OutputGroup) => void;
  updateOutputGroup: (groupId: string, updates: Partial<OutputGroup>) => void;
  removeOutputGroup: (groupId: string) => void;

  // Stream target mutations
  addStreamTarget: (groupId: string, target: StreamTarget) => void;
  updateStreamTarget: (groupId: string, targetId: string, updates: Partial<StreamTarget>) => void;
  removeStreamTarget: (groupId: string, targetId: string) => void;
}

export const useProfileStore = create<ProfileState>((set, get) => ({
  profiles: [],
  current: null,
  loading: false,
  error: null,

  setProfiles: (profiles) => set({ profiles }),
  setCurrentProfile: (profile) => set({ current: profile }),
  setLoading: (loading) => set({ loading }),
  setError: (error) => set({ error }),

  selectProfile: (id) => {
    // TODO: In Phase 4, this will call Tauri to load the full profile
    // For now, create a mock full profile from the summary
    const summary = get().profiles.find((p) => p.id === id);
    if (summary) {
      const mockProfile: Profile = {
        id: summary.id,
        name: summary.name,
        incomingUrl: 'rtmp://localhost/live',
        outputGroups: [],
      };
      set({ current: mockProfile });
    }
  },

  duplicateProfile: (id) => {
    // TODO: In Phase 4, this will call Tauri to duplicate the profile
    const summary = get().profiles.find((p) => p.id === id);
    if (summary) {
      const newProfile: ProfileSummary = {
        ...summary,
        id: crypto.randomUUID(),
        name: `${summary.name} (Copy)`,
      };
      set({ profiles: [...get().profiles, newProfile] });
    }
  },

  updateProfile: (updates) => {
    const current = get().current;
    if (current) {
      set({ current: { ...current, ...updates } });
    }
  },

  addOutputGroup: (group) => {
    const current = get().current;
    if (current) {
      set({
        current: {
          ...current,
          outputGroups: [...current.outputGroups, group],
        },
      });
    }
  },

  updateOutputGroup: (groupId, updates) => {
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
    }
  },

  removeOutputGroup: (groupId) => {
    const current = get().current;
    if (current) {
      set({
        current: {
          ...current,
          outputGroups: current.outputGroups.filter((g) => g.id !== groupId),
        },
      });
    }
  },

  addStreamTarget: (groupId, target) => {
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
    }
  },

  updateStreamTarget: (groupId, targetId, updates) => {
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
    }
  },

  removeStreamTarget: (groupId, targetId) => {
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
    }
  },
}));
