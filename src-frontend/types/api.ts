import type { Profile, OutputGroup } from './profile';
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
  load: (name: string, password?: string) => TauriResult<Profile>;
  save: (profile: Profile, password?: string) => TauriResult<void>;
  delete: (name: string) => TauriResult<void>;
  isEncrypted: (name: string) => TauriResult<boolean>;
}

/**
 * Stream API commands
 * @see src-frontend/lib/tauri.ts for implementation
 */
export interface StreamAPI {
  start: (group: OutputGroup, incomingUrl: string) => TauriResult<number>;
  stop: (groupId: string) => TauriResult<void>;
  stopAll: () => TauriResult<void>;
  getActiveCount: () => TauriResult<number>;
  isGroupStreaming: (groupId: string) => TauriResult<boolean>;
  getActiveGroupIds: () => TauriResult<string[]>;
}

/**
 * System API commands
 * @see src-frontend/lib/tauri.ts for implementation
 */
export interface SystemAPI {
  getEncoders: () => TauriResult<Encoders>;
  testFfmpeg: () => TauriResult<string>;
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
  lastProfile: string | null;
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
