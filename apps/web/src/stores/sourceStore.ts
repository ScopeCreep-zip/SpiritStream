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

// Cache TTL for device discovery (30 seconds)
const DEVICE_CACHE_TTL_MS = 30000;

interface SourceState {
  devices: DeviceDiscoveryState;
  error: string | null;

  // Actions
  discoverDevices: (force?: boolean) => Promise<void>;
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
  reorderSources: (profileName: string, sourceIds: string[], password?: string) => Promise<Source[]>;

  clearError: () => void;
}

export const useSourceStore = create<SourceState>((set, get) => ({
  devices: {
    cameras: [],
    displays: [],
    audioDevices: [],
    captureCards: [],
    isDiscovering: false,
    lastDiscovery: null,
  },
  error: null,

  discoverDevices: async (force = false) => {
    const { lastDiscovery, isDiscovering } = get().devices;

    // Skip if already discovering
    if (isDiscovering) return;

    // Skip if cache is fresh (within TTL) and not forced
    const isFresh = lastDiscovery && (Date.now() - lastDiscovery.getTime() < DEVICE_CACHE_TTL_MS);
    if (isFresh && !force) return;

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

  reorderSources: async (profileName, sourceIds, password) => {
    try {
      const sources = await api.source.reorder(profileName, sourceIds, password);
      return sources;
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  },

  clearError: () => set({ error: null }),
}));
