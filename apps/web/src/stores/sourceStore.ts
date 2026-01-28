/**
 * Source Store
 * Manages input sources for multi-input streaming
 */
import { create } from 'zustand';
import { api } from '@/lib/backend';
import type {
  Source,
  CameraDevice,
  DisplayInfo,
  AudioInputDevice,
  CaptureCardDevice,
} from '@/types/source';

interface DeviceDiscoveryState {
  cameras: CameraDevice[];
  displays: DisplayInfo[];
  audioDevices: AudioInputDevice[];
  captureCards: CaptureCardDevice[];
  isDiscovering: boolean;
  lastDiscovery: Date | null;
}

interface SourceState {
  devices: DeviceDiscoveryState;
  error: string | null;

  // Actions
  discoverDevices: () => Promise<void>;
  listCameras: () => Promise<CameraDevice[]>;
  listDisplays: () => Promise<DisplayInfo[]>;
  listAudioDevices: () => Promise<AudioInputDevice[]>;
  listCaptureCards: () => Promise<CaptureCardDevice[]>;

  // Source CRUD (operates on profile)
  addSource: (profileName: string, source: Source, password?: string) => Promise<Source[]>;
  updateSource: (
    profileName: string,
    sourceId: string,
    updates: Partial<Source>,
    password?: string
  ) => Promise<Source>;
  removeSource: (profileName: string, sourceId: string, password?: string) => Promise<void>;

  clearError: () => void;
}

export const useSourceStore = create<SourceState>((set) => ({
  devices: {
    cameras: [],
    displays: [],
    audioDevices: [],
    captureCards: [],
    isDiscovering: false,
    lastDiscovery: null,
  },
  error: null,

  discoverDevices: async () => {
    set((state) => ({
      devices: { ...state.devices, isDiscovering: true },
      error: null,
    }));

    try {
      const result = await api.device.refreshAll();

      set({
        devices: {
          cameras: result.cameras,
          displays: result.displays,
          audioDevices: result.audioDevices,
          captureCards: result.captureCards,
          isDiscovering: false,
          lastDiscovery: new Date(),
        },
      });
    } catch (err) {
      set((state) => ({
        devices: { ...state.devices, isDiscovering: false },
        error: err instanceof Error ? err.message : String(err),
      }));
    }
  },

  listCameras: async () => {
    try {
      const cameras = await api.device.listCameras();
      set((state) => ({
        devices: { ...state.devices, cameras },
      }));
      return cameras;
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      return [];
    }
  },

  listDisplays: async () => {
    try {
      const displays = await api.device.listDisplays();
      set((state) => ({
        devices: { ...state.devices, displays },
      }));
      return displays;
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      return [];
    }
  },

  listAudioDevices: async () => {
    try {
      const audioDevices = await api.device.listAudioDevices();
      set((state) => ({
        devices: { ...state.devices, audioDevices },
      }));
      return audioDevices;
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      return [];
    }
  },

  listCaptureCards: async () => {
    try {
      const captureCards = await api.device.listCaptureCards();
      set((state) => ({
        devices: { ...state.devices, captureCards },
      }));
      return captureCards;
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      return [];
    }
  },

  addSource: async (profileName, source, password) => {
    try {
      const sources = await api.source.add(profileName, source, password);
      return sources;
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  },

  updateSource: async (profileName, sourceId, updates, password) => {
    try {
      const source = await api.source.update(profileName, sourceId, updates, password);
      return source;
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  },

  removeSource: async (profileName, sourceId, password) => {
    try {
      await api.source.remove(profileName, sourceId, password);
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  },

  clearError: () => set({ error: null }),
}));
