/**
 * Tauri command result wrapper
 */
export type TauriResult<T> = Promise<T>;

/**
 * Global application settings (app-wide, not per-profile)
 *
 * Profile-specific settings (theme, language, integrations) have been moved
 * to ProfileSettings in profile.ts
 */
export interface AppSettings {
  // App-level behavior
  startMinimized: boolean;

  // System-wide FFmpeg configuration
  ffmpegPath: string;
  autoDownloadFfmpeg: boolean;

  // App-wide log management
  logRetentionDays: number;

  // Tracks which profile to load on startup
  lastProfile: string | null;
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
