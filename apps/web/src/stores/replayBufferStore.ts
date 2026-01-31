/**
 * Replay Buffer Store
 * Manages replay buffer state for instant replay saving
 * Communicates with the backend ReplayBufferService via HTTP API
 */
import { create } from 'zustand';
import { api } from '@/lib/backend/httpApi';

interface ReplayBufferStoreState {
  /** Whether the replay buffer is active (continuously recording) */
  isActive: boolean;
  /** Duration of the replay buffer in seconds */
  duration: number;
  /** Seconds currently buffered (0 to duration) */
  bufferedSeconds: number;
  /** Output directory for saved replays */
  outputPath: string;
  /** Whether currently saving a replay */
  isSaving: boolean;
  /** Whether an operation is in progress */
  isLoading: boolean;
  /** Last saved replay file path */
  lastSavedPath: string | null;
  /** Error message if any */
  error: string | null;
  /** Whether default paths have been initialized */
  initialized: boolean;

  /**
   * Initialize default output path from backend
   */
  initializeDefaultPath: () => Promise<void>;

  /**
   * Start the replay buffer
   */
  startBuffer: () => Promise<void>;

  /**
   * Stop the replay buffer
   */
  stopBuffer: () => Promise<void>;

  /**
   * Toggle replay buffer on/off
   */
  toggleBuffer: () => Promise<void>;

  /**
   * Save the current buffer contents to a file
   */
  saveReplay: () => Promise<void>;

  /**
   * Set the buffer duration in seconds
   */
  setDuration: (seconds: number) => Promise<void>;

  /**
   * Set the output path for saved replays
   */
  setOutputPath: (path: string) => Promise<void>;

  /**
   * Refresh state from the backend
   */
  refreshState: () => Promise<void>;

  /**
   * Clear error state
   */
  clearError: () => void;
}

// Polling interval for state updates (in ms)
const STATE_POLL_INTERVAL = 1000;
let pollInterval: ReturnType<typeof setInterval> | null = null;

function startStatePolling() {
  if (pollInterval) {
    clearInterval(pollInterval);
  }

  pollInterval = setInterval(async () => {
    try {
      const state = await api.replayBuffer.getState();
      useReplayBufferStore.setState({
        isActive: state.isActive,
        bufferedSeconds: state.bufferedSecs,
        duration: state.durationSecs,
        outputPath: state.outputPath || useReplayBufferStore.getState().outputPath,
      });
    } catch {
      // Silently ignore polling errors - the buffer might not be active
    }
  }, STATE_POLL_INTERVAL);
}

function stopStatePolling() {
  if (pollInterval) {
    clearInterval(pollInterval);
    pollInterval = null;
  }
}

// Fallback path used until platform-specific path is fetched
const FALLBACK_OUTPUT_PATH = '~/Videos/Replays';

export const useReplayBufferStore = create<ReplayBufferStoreState>((set, get) => ({
  isActive: false,
  duration: 30, // Default 30 seconds
  bufferedSeconds: 0,
  outputPath: FALLBACK_OUTPUT_PATH,
  isSaving: false,
  isLoading: false,
  lastSavedPath: null,
  error: null,
  initialized: false,

  initializeDefaultPath: async () => {
    // Only initialize once
    if (get().initialized) return;

    try {
      const paths = await api.system.getDefaultPaths();
      set({
        outputPath: paths.replays,
        initialized: true
      });
    } catch (err) {
      // Fallback to default on error - don't fail initialization
      console.warn('Failed to get default paths, using fallback:', err);
      set({ initialized: true });
    }
  },

  startBuffer: async () => {
    const { duration, outputPath } = get();

    set({ isLoading: true, error: null });

    try {
      await api.replayBuffer.start({
        durationSecs: duration,
        outputPath: outputPath,
        segmentDuration: 2, // 2 second segments for good balance
      });

      set({
        isActive: true,
        bufferedSeconds: 0,
        lastSavedPath: null,
        isLoading: false,
      });

      // Start polling for state updates
      startStatePolling();
    } catch (error) {
      set({
        isLoading: false,
        error: error instanceof Error ? error.message : 'Failed to start replay buffer',
      });
      throw error;
    }
  },

  stopBuffer: async () => {
    set({ isLoading: true, error: null });

    try {
      await api.replayBuffer.stop();

      set({
        isActive: false,
        bufferedSeconds: 0,
        isLoading: false,
      });

      // Stop polling
      stopStatePolling();
    } catch (error) {
      set({
        isLoading: false,
        error: error instanceof Error ? error.message : 'Failed to stop replay buffer',
      });
      throw error;
    }
  },

  toggleBuffer: async () => {
    const { isActive, startBuffer, stopBuffer } = get();
    if (isActive) {
      await stopBuffer();
    } else {
      await startBuffer();
    }
  },

  saveReplay: async () => {
    const { isActive, bufferedSeconds } = get();

    if (!isActive || bufferedSeconds === 0) {
      set({ error: 'No replay data to save' });
      return;
    }

    set({ isSaving: true, error: null });

    try {
      const result = await api.replayBuffer.save();

      set({
        lastSavedPath: result.filePath,
        isSaving: false,
      });
    } catch (error) {
      set({
        isSaving: false,
        error: error instanceof Error ? error.message : 'Failed to save replay',
      });
      throw error;
    }
  },

  setDuration: async (seconds) => {
    const clampedSeconds = Math.max(5, Math.min(300, seconds));

    // Update local state immediately for responsive UI
    set({ duration: clampedSeconds });

    try {
      await api.replayBuffer.setDuration(clampedSeconds);
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to set duration',
      });
      throw error;
    }
  },

  setOutputPath: async (path) => {
    // Update local state immediately for responsive UI
    set({ outputPath: path });

    try {
      await api.replayBuffer.setOutputPath(path);
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to set output path',
      });
      throw error;
    }
  },

  refreshState: async () => {
    try {
      const state = await api.replayBuffer.getState();
      set({
        isActive: state.isActive,
        bufferedSeconds: state.bufferedSecs,
        duration: state.durationSecs,
        outputPath: state.outputPath || get().outputPath,
        error: null,
      });

      // Start or stop polling based on active state
      if (state.isActive && !pollInterval) {
        startStatePolling();
      } else if (!state.isActive && pollInterval) {
        stopStatePolling();
      }
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to refresh state',
      });
    }
  },

  clearError: () => {
    set({ error: null });
  },
}));

// Initialize state on load
if (typeof window !== 'undefined') {
  // Attempt to sync with backend state on startup
  useReplayBufferStore.getState().refreshState().catch(() => {
    // Ignore errors on initial load - backend might not be ready
  });
}
