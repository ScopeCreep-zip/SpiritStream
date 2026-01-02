import type { Profile, OutputGroup, StreamTarget } from './profile';
import type { Encoders, StreamInfo } from './stream';

/**
 * Tauri command result wrapper
 */
export type TauriResult<T> = Promise<T>;

/**
 * Profile API commands
 */
export interface ProfileAPI {
  getAll: () => TauriResult<string[]>;
  load: (name: string, password?: string) => TauriResult<Profile>;
  save: (profile: Profile, password?: string) => TauriResult<void>;
  delete: (name: string) => TauriResult<void>;
  duplicate: (name: string, newName: string) => TauriResult<Profile>;
  setActive: (id: string) => TauriResult<void>;
  isEncrypted: (name: string) => TauriResult<boolean>;
}

/**
 * Stream API commands
 */
export interface StreamAPI {
  start: (group: OutputGroup, incomingUrl: string) => TauriResult<StreamInfo>;
  stop: (groupId: string) => TauriResult<void>;
  stopAll: () => TauriResult<void>;
  getStatus: () => TauriResult<Record<string, StreamInfo>>;
}

/**
 * Target API commands
 */
export interface TargetAPI {
  add: (groupId: string, target: StreamTarget) => TauriResult<void>;
  update: (groupId: string, targetId: string, target: Partial<StreamTarget>) => TauriResult<void>;
  remove: (groupId: string, targetId: string) => TauriResult<void>;
}

/**
 * System API commands
 */
export interface SystemAPI {
  getEncoders: () => TauriResult<Encoders>;
  testFfmpeg: () => TauriResult<string>;
  getFfmpegPath: () => TauriResult<string>;
  getFfmpegVersion: () => TauriResult<string>;
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
 * Settings API commands
 */
export interface SettingsAPI {
  get: () => TauriResult<AppSettings>;
  save: (settings: AppSettings) => TauriResult<void>;
  exportData: () => TauriResult<string>;
  clearData: () => TauriResult<void>;
}

/**
 * Complete API interface
 */
export interface TauriAPI {
  profile: ProfileAPI;
  stream: StreamAPI;
  target: TargetAPI;
  system: SystemAPI;
  settings: SettingsAPI;
}
