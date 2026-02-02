/**
 * Tauri command result wrapper
 */
export type TauriResult<T> = Promise<T>;

/**
 * Settings structure (matches Rust Settings model)
 */
export interface AppSettings {
  language: string;
  startMinimized: boolean;
  showNotifications: boolean;
  ffmpegPath: string;
  autoDownloadFfmpeg: boolean;
  encryptStreamKeys: boolean;
  logRetentionDays: number;
  themeId: string;
  backendRemoteEnabled: boolean;
  backendUiEnabled: boolean;
  backendHost: string;
  backendPort: number;
  backendToken: string;
  // OBS WebSocket settings
  obsHost: string;
  obsPort: number;
  obsPassword: string;
  obsUseAuth: boolean;
  obsDirection: ObsIntegrationDirection;
  obsAutoConnect: boolean;
  lastProfile: string | null;
  // Discord webhook settings
  discordWebhookEnabled: boolean;
  discordWebhookUrl: string;
  discordGoLiveMessage: string;
  discordCooldownEnabled: boolean;
  discordCooldownSeconds: number;
  discordImagePath: string;
  // Chat platform settings
  chatTwitchChannel: string;
  chatTwitchOauthToken: string;
  chatYoutubeChannelId: string;
  chatYoutubeApiKey: string;
  chatAutoConnect: boolean;
}

/**
 * Report returned after successful key rotation
 */
export interface RotationReport {
  profilesUpdated: number;
  keysReencrypted: number;
  totalProfiles: number;
  timestamp: string;
}

/**
 * Result of testing an RTMP target connection
 */
export interface RtmpTestResult {
  /** Whether the connection test was successful */
  success: boolean;
  /** Human-readable message describing the result */
  message: string;
  /** Time taken for the test in milliseconds */
  latency_ms: number | null;
}

/**
 * FFmpeg version information
 */
export interface FFmpegVersionInfo {
  /** Currently installed version (null if not installed) */
  installed_version: string | null;
  /** Latest available version for download */
  latest_version: string | null;
  /** Whether an update is available */
  update_available: boolean;
  /** Human-readable status message */
  status: string;
}

// ============================================================================
// OBS WebSocket Types
// ============================================================================

/**
 * OBS connection status
 */
export type ObsConnectionStatus = 'disconnected' | 'connecting' | 'connected' | 'error';

/**
 * OBS streaming status
 */
export type ObsStreamStatus = 'inactive' | 'starting' | 'active' | 'stopping' | 'unknown';

/**
 * OBS integration direction
 */
export type ObsIntegrationDirection =
  | 'obs-to-spiritstream'
  | 'spiritstream-to-obs'
  | 'bidirectional'
  | 'disabled';

/**
 * OBS WebSocket configuration
 */
export interface ObsConfig {
  host: string;
  port: number;
  password: string;
  useAuth: boolean;
  direction: ObsIntegrationDirection;
  autoConnect: boolean;
  /** Indicates if a password is set (password itself is masked) */
  hasPassword?: boolean;
}

/**
 * Current OBS state
 */
export interface ObsState {
  connectionStatus: ObsConnectionStatus;
  streamStatus: ObsStreamStatus;
  errorMessage: string | null;
  obsVersion: string | null;
  websocketVersion: string | null;
}
