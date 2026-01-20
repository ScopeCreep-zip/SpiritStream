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
