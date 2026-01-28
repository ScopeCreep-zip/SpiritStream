import { invoke } from '@tauri-apps/api/core';
import type { Profile, ProfileSummary, OutputGroup, RtmpInput } from '@/types/profile';
import type { Encoders } from '@/types/stream';
import type { AppSettings, FFmpegVersionInfo, RotationReport, RtmpTestResult } from '@/types/api';
import type { ThemeSummary } from '@/types/theme';
import type {
  Source,
  CameraDevice,
  DisplayInfo,
  AudioInputDevice,
  CaptureCardDevice,
} from '@/types/source';

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
    /** Stop a specific source preview */
    stopSourcePreview: (sourceId: string) =>
      invoke<void>('stop_source_preview', { sourceId }),
    /** Stop all active previews */
    stopAllPreviews: () => invoke<void>('stop_all_previews'),
  },
  device: {
    /** Refresh all device types at once */
    refreshAll: () =>
      invoke<{
        cameras: CameraDevice[];
        displays: DisplayInfo[];
        audioDevices: AudioInputDevice[];
        captureCards: CaptureCardDevice[];
      }>('refresh_devices'),
    /** List available cameras */
    listCameras: () => invoke<CameraDevice[]>('list_cameras'),
    /** List available displays for screen capture */
    listDisplays: () => invoke<DisplayInfo[]>('list_displays'),
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
    /** Remove a source from a profile. */
    remove: (profileName: string, sourceId: string, password?: string) =>
      invoke<void>('remove_source', { profileName, sourceId, password }),
  },
};
