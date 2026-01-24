/**
 * Scene Store
 * Manages scenes and scene composition for multi-input streaming
 */
import { create } from 'zustand';
import { api } from '@/lib/backend';
import type { Scene, SourceLayer, Transform } from '@/types/scene';

interface SceneState {
  selectedLayerId: string | null;
  error: string | null;

  // Actions - Scene CRUD
  createScene: (
    profileName: string,
    name: string,
    width?: number,
    height?: number,
    password?: string
  ) => Promise<string>;
  updateScene: (
    profileName: string,
    sceneId: string,
    updates: Partial<Scene>,
    password?: string
  ) => Promise<Scene>;
  deleteScene: (profileName: string, sceneId: string, password?: string) => Promise<void>;
  setActiveScene: (profileName: string, sceneId: string, password?: string) => Promise<void>;
  duplicateScene: (
    profileName: string,
    sceneId: string,
    newName?: string,
    password?: string
  ) => Promise<string>;

  // Actions - Layer management
  addLayer: (
    profileName: string,
    sceneId: string,
    sourceId: string,
    transform?: Transform,
    password?: string
  ) => Promise<string>;
  updateLayer: (
    profileName: string,
    sceneId: string,
    layerId: string,
    updates: Partial<SourceLayer>,
    password?: string
  ) => Promise<SourceLayer>;
  removeLayer: (
    profileName: string,
    sceneId: string,
    layerId: string,
    password?: string
  ) => Promise<void>;
  reorderLayers: (
    profileName: string,
    sceneId: string,
    layerIds: string[],
    password?: string
  ) => Promise<void>;

  // Actions - Audio mixer
  setTrackVolume: (
    profileName: string,
    sceneId: string,
    sourceId: string,
    volume: number,
    password?: string
  ) => Promise<void>;
  setTrackMuted: (
    profileName: string,
    sceneId: string,
    sourceId: string,
    muted: boolean,
    password?: string
  ) => Promise<void>;
  setTrackSolo: (
    profileName: string,
    sceneId: string,
    sourceId: string,
    solo: boolean,
    password?: string
  ) => Promise<void>;
  setMasterVolume: (
    profileName: string,
    sceneId: string,
    volume: number,
    password?: string
  ) => Promise<void>;

  // UI state
  selectLayer: (layerId: string | null) => void;
  clearError: () => void;
}

export const useSceneStore = create<SceneState>((set) => ({
  selectedLayerId: null,
  error: null,

  // Scene CRUD
  createScene: async (profileName, name, width, height, password) => {
    try {
      const result = await api.invoke<{ sceneId: string }>('create_scene', {
        profileName,
        name,
        width,
        height,
        password,
      });
      return result.sceneId;
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  },

  updateScene: async (profileName, sceneId, updates, password) => {
    try {
      const scene = await api.invoke<Scene>('update_scene', {
        profileName,
        sceneId,
        updates,
        password,
      });
      return scene;
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  },

  deleteScene: async (profileName, sceneId, password) => {
    try {
      await api.invoke('delete_scene', {
        profileName,
        sceneId,
        password,
      });
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  },

  setActiveScene: async (profileName, sceneId, password) => {
    try {
      await api.invoke('set_active_scene', {
        profileName,
        sceneId,
        password,
      });
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  },

  duplicateScene: async (profileName, sceneId, newName, password) => {
    try {
      const result = await api.invoke<{ sceneId: string }>('duplicate_scene', {
        profileName,
        sceneId,
        newName,
        password,
      });
      return result.sceneId;
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  },

  // Layer management
  addLayer: async (profileName, sceneId, sourceId, transform, password) => {
    try {
      const result = await api.invoke<{ layerId: string }>('add_layer_to_scene', {
        profileName,
        sceneId,
        sourceId,
        transform,
        password,
      });
      return result.layerId;
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  },

  updateLayer: async (profileName, sceneId, layerId, updates, password) => {
    try {
      const layer = await api.invoke<SourceLayer>('update_layer', {
        profileName,
        sceneId,
        layerId,
        updates,
        password,
      });
      return layer;
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  },

  removeLayer: async (profileName, sceneId, layerId, password) => {
    try {
      await api.invoke('remove_layer', {
        profileName,
        sceneId,
        layerId,
        password,
      });
      // Clear selection if removed layer was selected
      set((state) =>
        state.selectedLayerId === layerId ? { selectedLayerId: null } : state
      );
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  },

  reorderLayers: async (profileName, sceneId, layerIds, password) => {
    try {
      await api.invoke('reorder_layers', {
        profileName,
        sceneId,
        layerIds,
        password,
      });
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  },

  // Audio mixer
  setTrackVolume: async (profileName, sceneId, sourceId, volume, password) => {
    try {
      await api.invoke('set_track_volume', {
        profileName,
        sceneId,
        sourceId,
        volume,
        password,
      });
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  },

  setTrackMuted: async (profileName, sceneId, sourceId, muted, password) => {
    try {
      await api.invoke('set_track_muted', {
        profileName,
        sceneId,
        sourceId,
        muted,
        password,
      });
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  },

  setTrackSolo: async (profileName, sceneId, sourceId, solo, password) => {
    try {
      await api.invoke('set_track_solo', {
        profileName,
        sceneId,
        sourceId,
        solo,
        password,
      });
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  },

  setMasterVolume: async (profileName, sceneId, volume, password) => {
    try {
      await api.invoke('set_master_volume', {
        profileName,
        sceneId,
        volume,
        password,
      });
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  },

  // UI state
  selectLayer: (layerId) => set({ selectedLayerId: layerId }),
  clearError: () => set({ error: null }),
}));
