import { create } from 'zustand';
import { immer } from 'zustand/middleware/immer';
import { api } from '@/lib/backend';
import i18n from '@/lib/i18n';
import {
  type Profile,
  type ProfileSummary,
  type OutputGroup,
  type StreamTarget,
  type Platform,
  type Source,
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
  reloadProfile: () => Promise<void>;
  duplicateProfile: (name: string) => Promise<void>;

  // Source management (local state updates without auto-save)
  // Use these after source API calls to sync local state without reloading entire profile
  setCurrentSources: (sources: Source[]) => void;
  updateCurrentSource: (source: Source) => void;
  removeCurrentSource: (sourceId: string) => void;

  // Layer management (local state updates without auto-save)
  // Use these after layer API calls to sync local state without reloading entire profile
  updateCurrentLayer: (sceneId: string, layerId: string, updates: Partial<import('@/types/scene').SourceLayer>) => void;
  removeCurrentLayer: (sceneId: string, layerId: string) => void;
  reorderCurrentLayers: (sceneId: string, layerIds: string[]) => void;
  addCurrentLayer: (sceneId: string, layer: import('@/types/scene').SourceLayer) => void;
  /** Batch update multiple layers in a single state update (reduces re-renders during drag operations) */
  batchUpdateLayers: (sceneId: string, updates: Array<{ layerId: string; changes: Partial<import('@/types/scene').SourceLayer> }>) => void;

  // Audio mixer management (local state updates without auto-save)
  // Use these after audio API calls to sync local state without reloading entire profile
  addCurrentAudioTrack: (sceneId: string, track: import('@/types/scene').AudioTrack) => void;
  updateCurrentAudioTrack: (sceneId: string, sourceId: string, updates: Partial<import('@/types/scene').AudioTrack>) => void;
  updateCurrentMasterVolume: (sceneId: string, masterVolume: number) => void;
  updateCurrentMasterMuted: (sceneId: string, masterMuted: boolean) => void;

  // Scene management (local state updates without auto-save)
  // Use these after scene API calls to sync local state without reloading entire profile
  addCurrentScene: (scene: import('@/types/scene').Scene) => void;
  removeCurrentScene: (sceneId: string) => void;
  setCurrentActiveScene: (sceneId: string) => void;
  updateCurrentScene: (sceneId: string, updates: Partial<import('@/types/scene').Scene>) => void;

  // Profile mutations (local state updates only - no auto-save)
  // Use these after profile API calls to sync local state without reloading entire profile
  updateCurrentProfile: (updates: Partial<Profile>) => void;

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

export const useProfileStore = create<ProfileState>()(
  immer((set, get) => ({
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
      set({ profiles: summaries });
    } catch (error) {
      set({ error: String(error) });
    } finally {
      // Always clear loading state to prevent "Loading..." stuck on all pages
      set({ loading: false });
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
        // Trigger password modal - loading already cleared below in finally
        set({ pendingPasswordProfile: name });
        return;
      }

      const profile = await api.profile.load(name, password);
      set({
        current: profile,
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
        set({ passwordError: 'Incorrect password' });
      } else {
        set({ error: errorMsg, pendingPasswordProfile: null });
      }
    } finally {
      // Always clear loading state to prevent "Loading..." stuck on all pages
      set({ loading: false });
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

    // Don't set loading: true - this causes UI to flash "Loading..."
    // The caller should have already updated the state optimistically
    set({ error: null });
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
      set({ profiles: summaries });
    } catch (error) {
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
      });
    } catch (error) {
      set({ error: String(error) });
    } finally {
      // Always clear loading state
      set({ loading: false });
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

  // Update sources without triggering save (used after source API calls)
  setCurrentSources: (sources) => {
    set((state) => {
      if (state.current) state.current.sources = sources;
    });
  },

  // Update a single source locally without triggering save (used after updateSource API)
  updateCurrentSource: (source) => {
    set((state) => {
      if (!state.current) return;
      const idx = state.current.sources.findIndex((s) => s.id === source.id);
      if (idx !== -1) state.current.sources[idx] = source;
    });
  },

  // Remove a source locally without triggering save (used after removeSource API)
  // Also removes from all scenes (layers and audio tracks) to match backend behavior
  removeCurrentSource: (sourceId) => {
    set((state) => {
      if (!state.current) return;
      state.current.sources = state.current.sources.filter((s) => s.id !== sourceId);
      for (const scene of state.current.scenes) {
        scene.layers = scene.layers.filter((l) => l.sourceId !== sourceId);
        scene.audioMixer.tracks = scene.audioMixer.tracks.filter((t) => t.sourceId !== sourceId);
      }
    });
  },

  // Update a layer locally without triggering save (used after updateLayer API)
  updateCurrentLayer: (sceneId, layerId, updates) => {
    set((state) => {
      const scene = state.current?.scenes.find((s) => s.id === sceneId);
      if (!scene) return;
      const layer = scene.layers.find((l) => l.id === layerId);
      if (layer) Object.assign(layer, updates);
    });
  },

  // Remove a layer locally without triggering save (used after removeLayer API)
  removeCurrentLayer: (sceneId, layerId) => {
    set((state) => {
      const scene = state.current?.scenes.find((s) => s.id === sceneId);
      if (scene) {
        scene.layers = scene.layers.filter((l) => l.id !== layerId);
      }
    });
  },

  // Reorder layers locally without triggering save (used after reorderLayers API)
  reorderCurrentLayers: (sceneId, layerIds) => {
    set((state) => {
      const scene = state.current?.scenes.find((s) => s.id === sceneId);
      if (!scene) return;
      const layerMap = new Map(scene.layers.map((l) => [l.id, l]));
      const reordered: typeof scene.layers = [];
      for (let i = 0; i < layerIds.length; i++) {
        const layer = layerMap.get(layerIds[i]);
        if (layer) {
          layer.zIndex = i;
          reordered.push(layer);
        }
      }
      scene.layers = reordered;
    });
  },

  // Add a layer locally without triggering save (used after addLayer API)
  addCurrentLayer: (sceneId, layer) => {
    set((state) => {
      const scene = state.current?.scenes.find((s) => s.id === sceneId);
      if (scene) scene.layers.push(layer);
    });
  },

  // Batch update multiple layers in a single state update
  // This reduces re-renders during drag operations where multiple layers may be updated
  batchUpdateLayers: (sceneId, updates) => {
    if (updates.length === 0) return;
    set((state) => {
      const scene = state.current?.scenes.find((s) => s.id === sceneId);
      if (!scene) return;
      const updateMap = new Map(updates.map((u) => [u.layerId, u.changes]));
      for (const layer of scene.layers) {
        const changes = updateMap.get(layer.id);
        if (changes) Object.assign(layer, changes);
      }
    });
  },

  // Add an audio track locally without triggering save (used when adding audio sources)
  addCurrentAudioTrack: (sceneId, track) => {
    set((state) => {
      const scene = state.current?.scenes.find((s) => s.id === sceneId);
      if (scene) scene.audioMixer.tracks.push(track);
    });
  },

  // Update an audio track locally without triggering save (used after setTrackVolume/setTrackMuted/setTrackSolo API)
  updateCurrentAudioTrack: (sceneId, sourceId, updates) => {
    set((state) => {
      const scene = state.current?.scenes.find((s) => s.id === sceneId);
      if (!scene) return;
      const track = scene.audioMixer.tracks.find((t) => t.sourceId === sourceId);
      if (track) Object.assign(track, updates);
    });
  },

  // Update master volume locally without triggering save (used after setMasterVolume API)
  updateCurrentMasterVolume: (sceneId, masterVolume) => {
    set((state) => {
      const scene = state.current?.scenes.find((s) => s.id === sceneId);
      if (scene) scene.audioMixer.masterVolume = masterVolume;
    });
  },

  // Update master muted locally without triggering save (used after setMasterMuted API)
  updateCurrentMasterMuted: (sceneId, masterMuted) => {
    set((state) => {
      const scene = state.current?.scenes.find((s) => s.id === sceneId);
      if (scene) scene.audioMixer.masterMuted = masterMuted;
    });
  },

  // Add a scene locally without triggering save (used after createScene API)
  addCurrentScene: (scene) => {
    set((state) => {
      if (state.current) state.current.scenes.push(scene);
    });
  },

  // Remove a scene locally without triggering save (used after deleteScene API)
  removeCurrentScene: (sceneId) => {
    set((state) => {
      if (!state.current) return;
      state.current.scenes = state.current.scenes.filter((s) => s.id !== sceneId);
      if (state.current.activeSceneId === sceneId) {
        state.current.activeSceneId = state.current.scenes[0]?.id ?? '';
      }
    });
  },

  // Set active scene locally without triggering save (used after setActiveScene API)
  setCurrentActiveScene: (sceneId) => {
    set((state) => {
      if (state.current) state.current.activeSceneId = sceneId;
    });
  },

  // Update scene properties locally without triggering save (used after updateScene API)
  updateCurrentScene: (sceneId, updates) => {
    set((state) => {
      const scene = state.current?.scenes.find((s) => s.id === sceneId);
      if (scene) Object.assign(scene, updates);
    });
  },

  // Select and load a profile by name
  selectProfile: async (name) => {
    await get().loadProfile(name);
  },

  // Reload the current profile from backend (for refreshing after scene/source changes)
  reloadProfile: async () => {
    const current = get().current;
    if (current) {
      await get().loadProfile(current.name);
    }
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

  // Update profile properties locally without triggering save (used after profile API calls)
  updateCurrentProfile: (updates) => {
    set((state) => {
      if (state.current) Object.assign(state.current, updates);
    });
  },

  updateProfile: async (updates) => {
    set((state) => {
      if (state.current) Object.assign(state.current, updates);
    });
    await get().saveProfile();
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
    set((state) => {
      if (state.current) state.current.outputGroups.push(group);
    });
    await get().saveProfile();
  },

  updateOutputGroup: async (groupId, updates) => {
    set((state) => {
      const group = state.current?.outputGroups.find((g) => g.id === groupId);
      if (group) Object.assign(group, updates);
    });
    await get().saveProfile();
  },

  removeOutputGroup: async (groupId) => {
    const current = get().current;
    if (!current) return;
    const groupToDelete = current.outputGroups.find((g) => g.id === groupId);
    if (groupToDelete?.isDefault) {
      console.warn('Cannot delete the default passthrough output group');
      return;
    }
    set((state) => {
      if (state.current) {
        state.current.outputGroups = state.current.outputGroups.filter((g) => g.id !== groupId);
      }
    });
    await get().saveProfile();
  },

  addStreamTarget: async (groupId, target) => {
    set((state) => {
      const group = state.current?.outputGroups.find((g) => g.id === groupId);
      if (group) group.streamTargets.push(target);
    });
    await get().saveProfile();
  },

  updateStreamTarget: async (groupId, targetId, updates) => {
    set((state) => {
      const group = state.current?.outputGroups.find((g) => g.id === groupId);
      if (!group) return;
      const target = group.streamTargets.find((t) => t.id === targetId);
      if (target) Object.assign(target, updates);
    });
    await get().saveProfile();
  },

  removeStreamTarget: async (groupId, targetId) => {
    set((state) => {
      const group = state.current?.outputGroups.find((g) => g.id === groupId);
      if (group) {
        group.streamTargets = group.streamTargets.filter((t) => t.id !== targetId);
      }
    });
    await get().saveProfile();
  },

  moveStreamTarget: async (fromGroupId, toGroupId, targetId) => {
    if (fromGroupId === toGroupId) return;
    const current = get().current;
    if (!current) return;
    const sourceGroup = current.outputGroups.find((g) => g.id === fromGroupId);
    const target = sourceGroup?.streamTargets.find((t) => t.id === targetId);
    if (!target) return;

    set((state) => {
      if (!state.current) return;
      const from = state.current.outputGroups.find((g) => g.id === fromGroupId);
      const to = state.current.outputGroups.find((g) => g.id === toGroupId);
      if (from && to) {
        from.streamTargets = from.streamTargets.filter((t) => t.id !== targetId);
        to.streamTargets.push(target);
      }
    });
    await get().saveProfile();
  },
})));
