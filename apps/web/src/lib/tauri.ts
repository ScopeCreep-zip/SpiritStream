import { invoke } from '@tauri-apps/api/core';
import type { Profile, ProfileSummary, OutputGroup, RtmpInput } from '@/types/profile';
import type { Encoders } from '@/types/stream';
import type { AppSettings, FFmpegVersionInfo, RotationReport, RtmpTestResult } from '@/types/api';
import type { ThemeSummary } from '@/types/theme';
import type {
  Source,
  CameraDevice,
  DisplayInfo,
  WindowInfo,
  AudioInputDevice,
  CaptureCardDevice,
} from '@/types/source';

/** Result of remove_source command */
export type RemoveSourceResult =
  | { removed: true; linkedRemoved: string[] }
  | {
      requiresConfirmation: true;
      linkedSourceIds: string[];
      linkedSourceNames: string[];
      message: string;
    };

/**
 * Type-safe Tauri API wrapper
 */
export const api = {
  /**
   * Generic invoke method for calling any backend command.
   * Use this for commands that don't have a specific method on the api object.
   */
  invoke: <T>(command: string, args?: Record<string, unknown>): Promise<T> =>
    invoke<T>(command, args),

  profile: {
    getAll: () => invoke<string[]>('get_all_profiles'),
    /** Get profile summaries with services list for displaying platform icons (Story 1.1, 4.1, 4.2) */
    getSummaries: () => invoke<ProfileSummary[]>('get_profile_summaries'),
    load: (name: string, password?: string) => invoke<Profile>('load_profile', { name, password }),
    save: (profile: Profile, password?: string) =>
      invoke<void>('save_profile', { profile, password }),
    delete: (name: string) => invoke<void>('delete_profile', { name }),
    isEncrypted: (name: string) => invoke<boolean>('is_profile_encrypted', { name }),
    /** Validate RTMP input doesn't conflict with existing profiles (Story 2.2) */
    validateInput: (profileId: string, input: RtmpInput) =>
      invoke<void>('validate_input', { profileId, input }),
    setProfileOrder: (orderedNames: string[]) => 
      invoke<void>('set_profile_order', {orderedNames}),   
    getOrderIndexMap: () => invoke<Record<string, number>>('get_order_index_map'),
    ensureOrderIndexes: () => invoke<Record<string, number>>('ensure_order_indexes'),
  },
  stream: {
    start: (group: OutputGroup, incomingUrl: string) =>
      invoke<number>('start_stream', { group, incomingUrl }),
    startAll: (groups: OutputGroup[], incomingUrl: string) =>
      invoke<number[]>('start_all_streams', { groups, incomingUrl }),
    stop: (groupId: string) => invoke<void>('stop_stream', { groupId }),
    stopAll: () => invoke<void>('stop_all_streams'),
    getActiveCount: () => invoke<number>('get_active_stream_count'),
    isGroupStreaming: (groupId: string) => invoke<boolean>('is_group_streaming', { groupId }),
    getActiveGroupIds: () => invoke<string[]>('get_active_group_ids'),
    toggleTarget: (targetId: string, enabled: boolean, group: OutputGroup, incomingUrl: string) =>
      invoke<number>('toggle_stream_target', { targetId, enabled, group, incomingUrl }),
    isTargetDisabled: (targetId: string) => invoke<boolean>('is_target_disabled', { targetId }),
  },
  system: {
    getEncoders: () => invoke<Encoders>('get_encoders'),
    testFfmpeg: () => invoke<string>('test_ffmpeg'),
    getFfmpegPath: () => invoke<string | null>('get_bundled_ffmpeg_path'),
    checkFfmpegUpdate: (installedVersion?: string) =>
      invoke<FFmpegVersionInfo>('check_ffmpeg_update', { installedVersion }),
    /** Validate a custom FFmpeg path before saving */
    validateFfmpegPath: (path: string) => invoke<string>('validate_ffmpeg_path', { path }),
    /** Test RTMP target connectivity with actual connection attempt */
    testRtmpTarget: (url: string, streamKey: string) =>
      invoke<RtmpTestResult>('test_rtmp_target', { url, streamKey }),
    downloadFfmpeg: () => invoke<string>('download_ffmpeg'),
    cancelFfmpegDownload: () => invoke<void>('cancel_ffmpeg_download'),
    getRecentLogs: (maxLines?: number) =>
      invoke<string[]>('get_recent_logs', { maxLines }),
    exportLogs: (path: string, content: string) =>
      invoke<void>('export_logs', { path, content }),
    /** Get platform-specific default directories */
    getDefaultPaths: async (): Promise<DefaultPaths> => {
      const response = await fetch('http://127.0.0.1:8008/api/system/default-paths');
      const data = await response.json();
      if (!data.ok) throw new Error(data.error || 'Failed to get default paths');
      return data.data as DefaultPaths;
    },
    /** Health check endpoint - verifies backend is responding */
    health: async (): Promise<void> => {
      const response = await fetch('http://127.0.0.1:8008/health');
      if (!response.ok) throw new Error('Health check failed');
    },
  },
  settings: {
    get: () => invoke<AppSettings>('get_settings'),
    save: (settings: AppSettings) => invoke<void>('save_settings', { settings }),
    getProfilesPath: () => invoke<string>('get_profiles_path'),
    exportData: (exportPath: string) => invoke<void>('export_data', { exportPath }),
    clearData: () => invoke<void>('clear_data'),
    rotateMachineKey: () => invoke<RotationReport>('rotate_machine_key'),
  },
  theme: {
    list: () => invoke<ThemeSummary[]>('list_themes'),
    getTokens: (themeId: string) => invoke<Record<string, string>>('get_theme_tokens', { themeId }),
    install: (themePath: string) => invoke<ThemeSummary>('install_theme', { themePath }),
    refresh: () => invoke<ThemeSummary[]>('refresh_themes'),
  },
  preview: {
    /** Get the MJPEG preview URL for a source */
    getSourcePreviewUrl: (sourceId: string, width = 640, height = 360, fps = 15, quality = 10) => {
      const baseUrl = 'http://127.0.0.1:8008';
      return `${baseUrl}/api/preview/source/${sourceId}?width=${width}&height=${height}&fps=${fps}&quality=${quality}`;
    },
    /** Get a single snapshot URL for a source */
    getSourceSnapshotUrl: (sourceId: string, width = 640, height = 360, quality = 5) => {
      const baseUrl = 'http://127.0.0.1:8008';
      return `${baseUrl}/api/preview/source/${sourceId}/snapshot?width=${width}&height=${height}&quality=${quality}&t=${Date.now()}`;
    },
    /** Get the MJPEG preview URL for a composed scene (streaming) */
    getScenePreviewUrl: (
      profileName: string,
      sceneId: string,
      width = 1280,
      height = 720,
      fps = 15,
      quality = 5
    ) => {
      const baseUrl = 'http://127.0.0.1:8008';
      return `${baseUrl}/api/preview/scene/${encodeURIComponent(profileName)}/${sceneId}?width=${width}&height=${height}&fps=${fps}&quality=${quality}`;
    },
    /** Get a single snapshot URL for a composed scene */
    getSceneSnapshotUrl: (
      profileName: string,
      sceneId: string,
      width = 1280,
      height = 720,
      quality = 5
    ) => {
      const baseUrl = 'http://127.0.0.1:8008';
      return `${baseUrl}/api/preview/scene/${encodeURIComponent(profileName)}/${sceneId}/snapshot?width=${width}&height=${height}&quality=${quality}&t=${Date.now()}`;
    },
    /** Stop a specific source preview */
    stopSourcePreview: (sourceId: string) =>
      invoke<void>('stop_source_preview', { sourceId }),
    /** Stop the scene preview */
    stopScenePreview: () => invoke<void>('stop_scene_preview'),
    /** Stop all active previews */
    stopAllPreviews: () => invoke<void>('stop_all_previews'),
    /** Get URL for a static file (images, HTML) served by the backend */
    getStaticFileUrl: (filePath: string) => {
      const baseUrl = 'http://127.0.0.1:8008';
      return `${baseUrl}/api/static?path=${encodeURIComponent(filePath)}`;
    },
  },
  device: {
    /** Refresh all device types at once */
    refreshAll: () =>
      invoke<{
        cameras: CameraDevice[];
        displays: DisplayInfo[];
        windows: WindowInfo[];
        audioDevices: AudioInputDevice[];
        captureCards: CaptureCardDevice[];
      }>('refresh_devices'),
    /** List available cameras */
    listCameras: () => invoke<CameraDevice[]>('list_cameras'),
    /** List available displays for screen capture */
    listDisplays: () => invoke<DisplayInfo[]>('list_displays'),
    /** List available windows for window capture */
    listWindows: () => invoke<WindowInfo[]>('list_windows'),
    /** List available audio input devices */
    listAudioDevices: () => invoke<AudioInputDevice[]>('list_audio_devices'),
    /** List available capture cards */
    listCaptureCards: () => invoke<CaptureCardDevice[]>('list_capture_cards'),
  },
  source: {
    /** Add a source to a profile. Returns updated sources array. */
    add: (profileName: string, source: Source, password?: string) =>
      invoke<Source[]>('add_source', { profileName, source, password }),
    /** Update a source in a profile. Returns the updated source. */
    update: (profileName: string, sourceId: string, updates: Partial<Source>, password?: string) =>
      invoke<Source>('update_source', { profileName, sourceId, updates, password }),
    /**
     * Remove a source from a profile.
     * @param removeLinked - If false and linked sources exist, returns confirmation request.
     *                       If true or undefined, removes linked sources automatically.
     */
    remove: (profileName: string, sourceId: string, removeLinked?: boolean, password?: string) =>
      invoke<RemoveSourceResult>('remove_source', { profileName, sourceId, removeLinked, password }),
    /** Reorder sources in a profile. Returns updated sources array. */
    reorder: (profileName: string, sourceIds: string[], password?: string) =>
      invoke<Source[]>('reorder_sources', { profileName, sourceIds, password }),
  },
  recording: {
    /** Start recording to file */
    start: async (name: string, format?: string, encrypt?: boolean, password?: string): Promise<RecordingInfo> => {
      const baseUrl = 'http://127.0.0.1:8008';
      const response = await fetch(`${baseUrl}/api/recording/start`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'include',
        body: JSON.stringify({ name, format, encrypt, password }),
      });
      const data = await response.json();
      if (!data.ok) throw new Error(data.error || 'Failed to start recording');
      return data.data as RecordingInfo;
    },
    /** Stop recording */
    stop: async (): Promise<void> => {
      const baseUrl = 'http://127.0.0.1:8008';
      const response = await fetch(`${baseUrl}/api/recording/stop`, {
        method: 'POST',
        credentials: 'include',
      });
      const data = await response.json();
      if (!data.ok) throw new Error(data.error || 'Failed to stop recording');
    },
    /** List all recordings */
    list: async (): Promise<RecordingInfo[]> => {
      const baseUrl = 'http://127.0.0.1:8008';
      const response = await fetch(`${baseUrl}/api/recordings`, {
        credentials: 'include',
      });
      const data = await response.json();
      if (!data.ok) throw new Error(data.error || 'Failed to list recordings');
      return data.data as RecordingInfo[];
    },
    /** Delete a recording */
    delete: async (id: string): Promise<void> => {
      const baseUrl = 'http://127.0.0.1:8008';
      const response = await fetch(`${baseUrl}/api/recording/${id}`, {
        method: 'DELETE',
        credentials: 'include',
      });
      const data = await response.json();
      if (!data.ok) throw new Error(data.error || 'Failed to delete recording');
    },
    /** Export a recording */
    export: async (id: string, path: string): Promise<void> => {
      const baseUrl = 'http://127.0.0.1:8008';
      const response = await fetch(`${baseUrl}/api/recording/export`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'include',
        body: JSON.stringify({ id, path }),
      });
      const data = await response.json();
      if (!data.ok) throw new Error(data.error || 'Failed to export recording');
    },
  },
  audio: {
    /** Set the source IDs to monitor for audio levels. Returns capture status for each source. */
    setMonitorSources: (sourceIds: string[], profileName?: string) =>
      invoke<AudioMonitorResult>('set_audio_monitor_sources', { sourceIds, profileName }),
    /** Get audio monitor status */
    getMonitorStatus: () =>
      invoke<{ running: boolean }>('get_audio_monitor_status'),
    /** Get health status for all tracked sources. Healthy = received data in last 2 seconds. */
    getMonitorHealth: () =>
      invoke<AudioMonitorHealth>('get_audio_monitor_health'),
  },
  webrtc: {
    /** Check if go2rtc WebRTC server is available */
    isAvailable: async (): Promise<boolean> => {
      const baseUrl = 'http://127.0.0.1:8008';
      try {
        const response = await fetch(`${baseUrl}/api/webrtc/available`, {
          credentials: 'include',
        });
        const data = await response.json();
        return data.data as boolean;
      } catch {
        return false;
      }
    },
    /** Get WebRTC streaming info for a source */
    getInfo: async (sourceId: string): Promise<WebRtcInfo> => {
      const baseUrl = 'http://127.0.0.1:8008';
      const response = await fetch(`${baseUrl}/api/webrtc/info/${sourceId}`, {
        credentials: 'include',
      });
      const data = await response.json();
      return data.data as WebRtcInfo;
    },
    /** Start WebRTC streaming for a source */
    start: async (sourceId: string): Promise<WebRtcInfo> => {
      const baseUrl = 'http://127.0.0.1:8008';
      const response = await fetch(`${baseUrl}/api/webrtc/start/${sourceId}`, {
        method: 'POST',
        credentials: 'include',
      });
      const data = await response.json();
      if (!data.ok) throw new Error(data.error || 'Failed to start WebRTC stream');
      return data.data as WebRtcInfo;
    },
    /** Stop WebRTC streaming for a source */
    stop: async (sourceId: string): Promise<void> => {
      const baseUrl = 'http://127.0.0.1:8008';
      await fetch(`${baseUrl}/api/webrtc/stop/${sourceId}`, {
        method: 'POST',
        credentials: 'include',
      });
    },
  },
};

/** WebRTC streaming info returned by go2rtc */
export interface WebRtcInfo {
  available: boolean;
  whepUrl?: string;
  wsUrl?: string;
  streamName?: string;
}

/** Platform-specific default directories */
export interface DefaultPaths {
  platform: 'macos' | 'windows' | 'linux';
  home: string;
  videos: string;
  recordings: string;
  replays: string;
}

/** Recording info returned by the recording API */
export interface RecordingInfo {
  id: string;
  name: string;
  filePath: string;
  format: string;
  durationSecs?: number;
  fileSizeBytes?: number;
  createdAt: string;
  encrypted: boolean;
}

/** Result of audio capture for a single source */
export interface AudioCaptureResult {
  success: boolean;
  sourceType?: string;
  deviceName?: string;
  reason?: 'notImplemented' | 'captureFailed' | 'notFound' | 'profileLoadFailed' | 'noAudio' | 'unsupportedFormat' | 'platformLimitation' | 'extractionUnavailable' | 'noCurrentItem' | 'extractionFailed';
  message?: string;
}

/** Result of set_audio_monitor_sources command */
export interface AudioMonitorResult {
  captureResults: Record<string, AudioCaptureResult>;
  trackedSources: number;
}

/** Health status for tracked audio sources */
export interface AudioMonitorHealth {
  /** Map of sourceId -> healthy (true if received data in last 2 seconds) */
  sources: Record<string, boolean>;
}
