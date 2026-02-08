import { invoke } from '@tauri-apps/api/core';
import type { Profile, ProfileSummary, OutputGroup, RtmpInput } from '@/types/profile';
import type { Encoders } from '@/types/stream';
import type {
  AppSettings,
  FFmpegVersionInfo,
  ObsConfig,
  ObsIntegrationDirection,
  ObsState,
  RotationReport,
  RtmpTestResult,
} from '@/types/api';
import type { ThemeSummary } from '@/types/theme';
import type {
  ChatConfig,
  ChatLogStatus,
  ChatPlatform,
  ChatPlatformStatus,
  ChatSendResult,
  ChatMessage,
} from '@/types/chat';

/**
 * Type-safe Tauri API wrapper
 */
export const api = {
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
    /** Retry a failed stream. Returns PID and next delay if another retry is needed */
    retry: (groupId: string) =>
      invoke<{ pid: number; nextDelaySecs: number | null }>('retry_stream', { groupId }),
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
    deleteFfmpeg: () => invoke<void>('delete_ffmpeg'),
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
  obs: {
    getState: () => invoke<ObsState>('obs_get_state'),
    getConfig: () => invoke<ObsConfig>('obs_get_config'),
    setConfig: (config: {
      host: string;
      port: number;
      password?: string;
      useAuth: boolean;
      direction: ObsIntegrationDirection;
      autoConnect: boolean;
    }) => invoke<void>('obs_set_config', config),
    connect: () => invoke<void>('obs_connect'),
    disconnect: () => invoke<void>('obs_disconnect'),
    startStream: () => invoke<void>('obs_start_stream'),
    stopStream: () => invoke<void>('obs_stop_stream'),
    isConnected: () => invoke<boolean>('obs_is_connected'),
  },
  discord: {
    testWebhook: (url: string) =>
      invoke<{ success: boolean; message: string; skippedCooldown: boolean }>(
        'discord_test_webhook',
        { url }
      ),
    sendNotification: () =>
      invoke<{ success: boolean; message: string; skippedCooldown: boolean }>(
        'discord_send_notification'
      ),
    resetCooldown: () => invoke<void>('discord_reset_cooldown'),
  },
  chat: {
    connect: (config: ChatConfig) => invoke<void>('connect_chat', { config }),
    sendMessage: (message: string) => invoke<ChatSendResult[]>('send_chat_message', { message }),
    disconnect: (platform: ChatPlatform) => invoke<void>('disconnect_chat', { platform }),
    retryConnection: (platform: ChatPlatform) => invoke<void>('retry_chat_connection', { platform }),
    disconnectAll: () => invoke<void>('disconnect_all_chat'),
    getStatus: () => invoke<ChatPlatformStatus[]>('get_chat_status'),
    getLogStatus: () => invoke<ChatLogStatus>('chat_get_log_status'),
    exportLog: (path: string) => invoke<void>('chat_export_log', { path }),
    searchSession: (query: string, limit?: number) =>
      invoke<ChatMessage[]>('chat_search_session', { query, limit }),
    getPlatformStatus: (platform: ChatPlatform) =>
      invoke<ChatPlatformStatus | null>('get_platform_chat_status', { platform }),
    isConnected: () => invoke<boolean>('is_chat_connected'),
  },
  oauth: {
    /** Check if a provider is configured (always true with embedded client IDs) */
    isConfigured: (provider: string) => invoke<boolean>('oauth_is_configured', { provider }),
    /** Start the OAuth flow for a provider - opens browser */
    startFlow: (provider: string) =>
      invoke<{ authUrl: string; callbackPort: number; state: string }>('oauth_start_flow', {
        provider,
      }),
    /** Complete the OAuth flow: exchange code, fetch user info, store tokens */
    completeFlow: (provider: string, code: string, state: string) =>
      invoke<{
        provider: string;
        userId: string;
        username: string;
        displayName: string;
      }>('oauth_complete_flow', { provider, code, state }),
    /** Get stored OAuth account info for a provider */
    getAccount: (provider: string) =>
      invoke<{
        loggedIn: boolean;
        userId?: string;
        username?: string;
        displayName?: string;
      }>('oauth_get_account', { provider }),
    /** Disconnect from a provider (clears tokens but doesn't revoke) */
    disconnect: (provider: string) => invoke<void>('oauth_disconnect', { provider }),
    /** Forget account (revokes tokens and clears from settings) */
    forget: (provider: string) => invoke<void>('oauth_forget', { provider }),
    /** Refresh an access token */
    refreshToken: (provider: string, refreshToken: string) =>
      invoke<{
        accessToken: string;
        refreshToken?: string;
        expiresIn?: number;
      }>('oauth_refresh_token', { provider, refreshToken }),
    /** Get OAuth configuration status (always configured with embedded IDs) */
    getConfig: () =>
      invoke<{ twitchConfigured: boolean; youtubeConfigured: boolean }>('oauth_get_config'),
    /** Set OAuth configuration (for users who want to use their own credentials) */
    setConfig: (config: {
      twitchClientId?: string;
      twitchClientSecret?: string;
      youtubeClientId?: string;
      youtubeClientSecret?: string;
    }) => invoke<void>('oauth_set_config', { config }),
  },
};
