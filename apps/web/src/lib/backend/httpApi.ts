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
import { getBackendBaseUrl, safeFetch } from './env';

interface InvokeOk<T> {
  ok: true;
  data: T;
}

interface InvokeError {
  ok: false;
  error: string;
}

type InvokeResponse<T> = InvokeOk<T> | InvokeError;

type InvokeArgs = Record<string, unknown> | undefined;

async function invokeHttp<T>(command: string, args?: InvokeArgs): Promise<T> {
  const baseUrl = getBackendBaseUrl();
  const url = `${baseUrl}/api/invoke/${command}`;

  const response = await safeFetch(url, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    credentials: 'include', // Send cookies for authentication
    body: JSON.stringify(args ?? {}),
  });

  const text = await response.text();

  // Safely parse JSON response, handling malformed responses gracefully
  let parsed: InvokeResponse<T> | T | undefined;
  if (text) {
    try {
      parsed = JSON.parse(text) as InvokeResponse<T> | T;
    } catch {
      throw new Error('Invalid response from server');
    }
  }

  if (!response.ok) {
    const errorMessage =
      parsed && typeof parsed === 'object' && 'error' in parsed
        ? String((parsed as InvokeError).error)
        : response.statusText || 'Request failed';
    throw new Error(errorMessage);
  }

  if (parsed && typeof parsed === 'object' && 'ok' in parsed) {
    if (!parsed.ok) {
      throw new Error(parsed.error || 'Request failed');
    }
    return parsed.data;
  }

  return parsed as T;
}

/**
 * HTTP API wrapper that mirrors the Tauri command surface.
 * All requests include credentials (cookies) for authentication.
 */
export const api = {
  /**
   * Generic invoke method for calling any backend command.
   * Use this for commands that don't have a specific method on the api object.
   */
  invoke: <T>(command: string, args?: InvokeArgs): Promise<T> => invokeHttp<T>(command, args),

  profile: {
    getAll: () => invokeHttp<string[]>('get_all_profiles'),
    getSummaries: () => invokeHttp<ProfileSummary[]>('get_profile_summaries'),
    load: (name: string, password?: string) =>
      invokeHttp<Profile>('load_profile', { name, password }),
    save: (profile: Profile, password?: string) =>
      invokeHttp<void>('save_profile', { profile, password }),
    delete: (name: string) => invokeHttp<void>('delete_profile', { name }),
    isEncrypted: (name: string) => invokeHttp<boolean>('is_profile_encrypted', { name }),
    validateInput: (profileId: string, input: RtmpInput) =>
      invokeHttp<void>('validate_input', { profileId, input }),
    setProfileOrder: (orderedNames: string[]) =>
      invokeHttp<void>('set_profile_order', { orderedNames }),
    getOrderIndexMap: () => invokeHttp<Record<string, number>>('get_order_index_map'),
    ensureOrderIndexes: () => invokeHttp<Record<string, number>>('ensure_order_indexes'),
  },
  stream: {
    /** Start streaming for a single output group. Returns the FFmpeg process PID */
    start: (group: OutputGroup, incomingUrl: string) =>
      invokeHttp<number>('start_stream', { group, incomingUrl }),
    /** Start all output groups. Returns array of FFmpeg process PIDs */
    startAll: (groups: OutputGroup[], incomingUrl: string) =>
      invokeHttp<number[]>('start_all_streams', { groups, incomingUrl }),
    /** Stop streaming for a specific output group */
    stop: (groupId: string) => invokeHttp<void>('stop_stream', { groupId }),
    /** Stop all active streams */
    stopAll: () => invokeHttp<void>('stop_all_streams'),
    getActiveCount: () => invokeHttp<number>('get_active_stream_count'),
    isGroupStreaming: (groupId: string) =>
      invokeHttp<boolean>('is_group_streaming', { groupId }),
    getActiveGroupIds: () => invokeHttp<string[]>('get_active_group_ids'),
    toggleTarget: (targetId: string, enabled: boolean, group: OutputGroup, incomingUrl: string) =>
      invokeHttp<number>('toggle_stream_target', { targetId, enabled, group, incomingUrl }),
    isTargetDisabled: (targetId: string) =>
      invokeHttp<boolean>('is_target_disabled', { targetId }),
  },
  system: {
    /** Get available video and audio encoders detected on the system */
    getEncoders: () => invokeHttp<Encoders>('get_encoders'),
    /** Test FFmpeg installation. Returns version string (e.g., "ffmpeg version 6.0") on success */
    testFfmpeg: () => invokeHttp<string>('test_ffmpeg'),
    /** Get path to bundled FFmpeg binary, or null if not bundled */
    getFfmpegPath: () => invokeHttp<string | null>('get_bundled_ffmpeg_path'),
    checkFfmpegUpdate: (installedVersion?: string) =>
      invokeHttp<FFmpegVersionInfo>('check_ffmpeg_update', { installedVersion }),
    validateFfmpegPath: (path: string) => invokeHttp<string>('validate_ffmpeg_path', { path }),
    testRtmpTarget: (url: string, streamKey: string) =>
      invokeHttp<RtmpTestResult>('test_rtmp_target', { url, streamKey }),
    getRecentLogs: (maxLines?: number) =>
      invokeHttp<string[]>('get_recent_logs', { maxLines }),
    exportLogs: (path: string, content: string) =>
      invokeHttp<void>('export_logs', { path, content }),
    downloadFfmpeg: () => invokeHttp<string>('download_ffmpeg'),
    cancelFfmpegDownload: () => invokeHttp<void>('cancel_ffmpeg_download'),
  },
  settings: {
    get: () => invokeHttp<AppSettings>('get_settings'),
    save: (settings: AppSettings) => invokeHttp<void>('save_settings', { settings }),
    getProfilesPath: () => invokeHttp<string>('get_profiles_path'),
    exportData: (exportPath: string) => invokeHttp<void>('export_data', { exportPath }),
    clearData: () => invokeHttp<void>('clear_data'),
    rotateMachineKey: () => invokeHttp<RotationReport>('rotate_machine_key'),
  },
  theme: {
    list: () => invokeHttp<ThemeSummary[]>('list_themes'),
    getTokens: (themeId: string) =>
      invokeHttp<Record<string, string>>('get_theme_tokens', { themeId }),
    install: (themePath: string) => invokeHttp<ThemeSummary>('install_theme', { themePath }),
    refresh: () => invokeHttp<ThemeSummary[]>('refresh_themes'),
  },
  preview: {
    /** Get the MJPEG preview URL for a source (streaming - may not work in WebKit) */
    getSourcePreviewUrl: (sourceId: string, width = 640, height = 360, fps = 15, quality = 5) => {
      const baseUrl = getBackendBaseUrl();
      return `${baseUrl}/api/preview/source/${sourceId}?width=${width}&height=${height}&fps=${fps}&quality=${quality}`;
    },
    /** Get a single snapshot URL for a source (works in all browsers) */
    getSourceSnapshotUrl: (sourceId: string, width = 640, height = 360, quality = 5) => {
      const baseUrl = getBackendBaseUrl();
      // Add timestamp to prevent caching
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
      const baseUrl = getBackendBaseUrl();
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
      const baseUrl = getBackendBaseUrl();
      // Add timestamp to prevent caching
      return `${baseUrl}/api/preview/scene/${encodeURIComponent(profileName)}/${sceneId}/snapshot?width=${width}&height=${height}&quality=${quality}&t=${Date.now()}`;
    },
    /** Stop a specific source preview */
    stopSourcePreview: (sourceId: string) =>
      invokeHttp<void>('stop_source_preview', { sourceId }),
    /** Stop the scene preview */
    stopScenePreview: () => invokeHttp<void>('stop_scene_preview'),
    /** Stop all active previews */
    stopAllPreviews: () => invokeHttp<void>('stop_all_previews'),
    /** Get URL for a static file (images, HTML) served by the backend */
    getStaticFileUrl: (filePath: string) => {
      const baseUrl = getBackendBaseUrl();
      return `${baseUrl}/api/static?path=${encodeURIComponent(filePath)}`;
    },
  },
  device: {
    /** Refresh all device types at once */
    refreshAll: () =>
      invokeHttp<{
        cameras: CameraDevice[];
        displays: DisplayInfo[];
        audioDevices: AudioInputDevice[];
        captureCards: CaptureCardDevice[];
      }>('refresh_devices'),
    /** List available cameras */
    listCameras: () => invokeHttp<CameraDevice[]>('list_cameras'),
    /** List available displays for screen capture */
    listDisplays: () => invokeHttp<DisplayInfo[]>('list_displays'),
    /** List available audio input devices */
    listAudioDevices: () => invokeHttp<AudioInputDevice[]>('list_audio_devices'),
    /** List available capture cards */
    listCaptureCards: () => invokeHttp<CaptureCardDevice[]>('list_capture_cards'),
  },
  source: {
    /** Add a source to a profile. Returns updated sources array. */
    add: (profileName: string, source: Source, password?: string) =>
      invokeHttp<Source[]>('add_source', { profileName, source, password }),
    /** Update a source in a profile. Returns the updated source. */
    update: (profileName: string, sourceId: string, updates: Partial<Source>, password?: string) =>
      invokeHttp<Source>('update_source', { profileName, sourceId, updates, password }),
    /** Remove a source from a profile. */
    remove: (profileName: string, sourceId: string, password?: string) =>
      invokeHttp<void>('remove_source', { profileName, sourceId, password }),
    /** Reorder sources in a profile. Returns updated sources array. */
    reorder: (profileName: string, sourceIds: string[], password?: string) =>
      invokeHttp<Source[]>('reorder_sources', { profileName, sourceIds, password }),
  },
  webrtc: {
    /** Check if go2rtc WebRTC server is available */
    isAvailable: async () => {
      const baseUrl = getBackendBaseUrl();
      const response = await safeFetch(`${baseUrl}/api/webrtc/available`, {
        credentials: 'include',
      });
      const data = await response.json();
      return data.data as boolean;
    },
    /** Get WebRTC streaming info for a source */
    getInfo: async (sourceId: string) => {
      const baseUrl = getBackendBaseUrl();
      const response = await safeFetch(`${baseUrl}/api/webrtc/info/${sourceId}`, {
        credentials: 'include',
      });
      const data = await response.json();
      return data.data as WebRtcInfo;
    },
    /** Start WebRTC streaming for a source */
    start: async (sourceId: string) => {
      const baseUrl = getBackendBaseUrl();
      const response = await safeFetch(`${baseUrl}/api/webrtc/start/${sourceId}`, {
        method: 'POST',
        credentials: 'include',
      });
      const data = await response.json();
      if (!data.ok) throw new Error(data.error || 'Failed to start WebRTC stream');
      return data.data as WebRtcInfo;
    },
    /** Stop WebRTC streaming for a source */
    stop: async (sourceId: string) => {
      const baseUrl = getBackendBaseUrl();
      await safeFetch(`${baseUrl}/api/webrtc/stop/${sourceId}`, {
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
