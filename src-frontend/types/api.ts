import type { Profile, OutputGroup, ProfileSummary, RtmpInput } from './profile';
import type { Encoders } from './stream';
import type { ThemeSummary, ThemeTokens } from './theme';

/**
 * Tauri command result wrapper
 */
export type TauriResult<T> = Promise<T>;

/**
 * Profile API commands
 * @see src-frontend/lib/tauri.ts for implementation
 */
export interface ProfileAPI {
  getAll: () => TauriResult<string[]>;
  getSummaries: () => TauriResult<ProfileSummary[]>;
  load: (name: string, password?: string) => TauriResult<Profile>;
  save: (profile: Profile, password?: string) => TauriResult<void>;
  delete: (name: string) => TauriResult<void>;
  isEncrypted: (name: string) => TauriResult<boolean>;
  validateInput: (profileId: string, input: RtmpInput) => TauriResult<void>;
}

/**
 * Stream API commands
 * @see src-frontend/lib/tauri.ts for implementation
 */
export interface StreamAPI {
  start: (group: OutputGroup, incomingUrl: string) => TauriResult<number>;
  startAll: (groups: OutputGroup[], incomingUrl: string) => TauriResult<number[]>;
  stop: (groupId: string) => TauriResult<void>;
  stopAll: () => TauriResult<void>;
  getActiveCount: () => TauriResult<number>;
  isGroupStreaming: (groupId: string) => TauriResult<boolean>;
  getActiveGroupIds: () => TauriResult<string[]>;
  toggleTarget: (
    targetId: string,
    enabled: boolean,
    group: OutputGroup,
    incomingUrl: string
  ) => TauriResult<number>;
  isTargetDisabled: (targetId: string) => TauriResult<boolean>;
}

/**
 * System API commands
 * @see src-frontend/lib/tauri.ts for implementation
 */
export interface SystemAPI {
  getEncoders: () => TauriResult<Encoders>;
  testFfmpeg: () => TauriResult<string>;
  getFfmpegPath: () => TauriResult<string | null>;
  checkFfmpegUpdate: (installedVersion?: string) => TauriResult<FFmpegVersionInfo>;
  validateFfmpegPath: (path: string) => TauriResult<string>;
  getRecentLogs: (maxLines?: number) => TauriResult<string[]>;
  exportLogs: (path: string, content: string) => TauriResult<void>;
  downloadFfmpeg: () => TauriResult<string>;
  cancelFfmpegDownload: () => TauriResult<void>;
}

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

/**
 * Settings API commands
 * @see src-frontend/lib/tauri.ts for implementation
 */
export interface SettingsAPI {
  get: () => TauriResult<AppSettings>;
  save: (settings: AppSettings) => TauriResult<void>;
  getProfilesPath: () => TauriResult<string>;
  exportData: (exportPath: string) => TauriResult<void>;
  clearData: () => TauriResult<void>;
  rotateMachineKey: () => TauriResult<RotationReport>;
}

/**
 * Theme API commands
 * @see src-frontend/lib/tauri.ts for implementation
 */
export interface ThemeAPI {
  list: () => TauriResult<ThemeSummary[]>;
  getTokens: (themeId: string) => TauriResult<ThemeTokens>;
  install: (themePath: string) => TauriResult<ThemeSummary>;
}

/**
 * Complete API interface
 * Note: Targets are managed via Profile mutations, not a separate API
 */
export interface TauriAPI {
  profile: ProfileAPI;
  stream: StreamAPI;
  system: SystemAPI;
  settings: SettingsAPI;
  theme: ThemeAPI;
}
