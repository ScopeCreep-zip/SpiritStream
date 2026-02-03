/**
 * Recording Store
 * Manages local recording state
 */
import { create } from 'zustand';
import { api } from '@/lib/backend';

export type RecordingFormat = 'mp4' | 'mkv' | 'mov' | 'webm';

interface RecordingState {
  /** Whether recording is currently active */
  isRecording: boolean;
  /** Recording duration in seconds */
  duration: number;
  /** Output file path */
  outputPath: string;
  /** Recording format */
  format: RecordingFormat;
  /** Recording error if any */
  error: string | null;
  /** Whether default paths have been initialized */
  initialized: boolean;

  // Actions
  initializeDefaultPath: () => Promise<void>;
  startRecording: (outputPath?: string, format?: RecordingFormat) => Promise<void>;
  stopRecording: () => Promise<void>;
  setOutputPath: (path: string) => void;
  setFormat: (format: RecordingFormat) => void;
  incrementDuration: () => void;
  reset: () => void;
}

// Fallback path used until platform-specific path is fetched
const FALLBACK_OUTPUT_PATH = '~/Videos';

export const useRecordingStore = create<RecordingState>((set, get) => ({
  isRecording: false,
  duration: 0,
  outputPath: FALLBACK_OUTPUT_PATH,
  format: 'mp4',
  error: null,
  initialized: false,

  initializeDefaultPath: async () => {
    // Only initialize once
    if (get().initialized) return;

    try {
      const paths = await api.system.getDefaultPaths();
      set({
        outputPath: paths.recordings,
        initialized: true
      });
    } catch (err) {
      // Fallback to default on error - don't fail initialization
      console.warn('Failed to get default paths, using fallback:', err);
      set({ initialized: true });
    }
  },

  startRecording: async (outputPath, format) => {
    const state = get();
    const finalPath = outputPath || state.outputPath;
    const finalFormat = format || state.format;

    try {
      // Generate recording name based on timestamp
      const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
      const recordingName = `recording-${timestamp}`;
      await api.recording.start(recordingName, finalFormat);
      set({
        isRecording: true,
        duration: 0,
        outputPath: finalPath,
        format: finalFormat,
        error: null,
      });
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  },

  stopRecording: async () => {
    try {
      await api.recording.stop();
      set({
        isRecording: false,
      });
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  },

  setOutputPath: (path) => set({ outputPath: path }),

  setFormat: (format) => set({ format }),

  incrementDuration: () => set((state) => ({ duration: state.duration + 1 })),

  reset: () =>
    set({
      isRecording: false,
      duration: 0,
      error: null,
    }),
}));

/**
 * Format recording duration as HH:MM:SS
 */
export function formatRecordingDuration(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;

  if (hours > 0) {
    return `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
  }
  return `${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
}

/**
 * Recording format options
 */
export const RECORDING_FORMATS: { value: RecordingFormat; label: string }[] = [
  { value: 'mp4', label: 'MP4 (H.264)' },
  { value: 'mkv', label: 'MKV (Matroska)' },
  { value: 'mov', label: 'MOV (QuickTime)' },
  { value: 'webm', label: 'WebM (VP9)' },
];
