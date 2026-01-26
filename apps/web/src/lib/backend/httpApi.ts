import type { Profile, ProfileSummary, OutputGroup, RtmpInput } from '@/types/profile';
import type { Encoders } from '@/types/stream';
import type {
  AppSettings,
  FFmpegVersionInfo,
  RotationReport,
  RtmpTestResult,
  ObsConfig,
  ObsState,
  ObsIntegrationDirection,
} from '@/types/api';
import type { ThemeSummary } from '@/types/theme';
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
    deleteFfmpeg: () => invokeHttp<void>('delete_ffmpeg'),
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
  obs: {
    /** Get current OBS connection and stream state */
    getState: () => invokeHttp<ObsState>('obs_get_state'),
    /** Get OBS WebSocket configuration (password is masked) */
    getConfig: () => invokeHttp<ObsConfig>('obs_get_config'),
    /** Update OBS WebSocket configuration */
    setConfig: (config: {
      host: string;
      port: number;
      password?: string;
      useAuth: boolean;
      direction: ObsIntegrationDirection;
      autoConnect: boolean;
    }) => invokeHttp<void>('obs_set_config', config),
    /** Connect to OBS WebSocket server */
    connect: () => invokeHttp<void>('obs_connect'),
    /** Disconnect from OBS WebSocket server */
    disconnect: () => invokeHttp<void>('obs_disconnect'),
    /** Start streaming in OBS */
    startStream: () => invokeHttp<void>('obs_start_stream'),
    /** Stop streaming in OBS */
    stopStream: () => invokeHttp<void>('obs_stop_stream'),
    /** Check if connected to OBS */
    isConnected: () => invokeHttp<boolean>('obs_is_connected'),
  },
};
