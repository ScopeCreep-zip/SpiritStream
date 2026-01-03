import { invoke } from '@tauri-apps/api/core';
import type { Profile, OutputGroup } from '@/types/profile';
import type { Encoders } from '@/types/stream';
import type { AppSettings } from '@/types/api';

/**
 * Type-safe Tauri API wrapper
 */
export const api = {
  profile: {
    getAll: () => invoke<string[]>('get_all_profiles'),
    load: (name: string, password?: string) =>
      invoke<Profile>('load_profile', { name, password }),
    save: (profile: Profile, password?: string) =>
      invoke<void>('save_profile', { profile, password }),
    delete: (name: string) => invoke<void>('delete_profile', { name }),
    isEncrypted: (name: string) => invoke<boolean>('is_profile_encrypted', { name }),
  },
  stream: {
    start: (group: OutputGroup, incomingUrl: string) =>
      invoke<number>('start_stream', { group, incomingUrl }),
    stop: (groupId: string) => invoke<void>('stop_stream', { groupId }),
    stopAll: () => invoke<void>('stop_all_streams'),
    getActiveCount: () => invoke<number>('get_active_stream_count'),
    isGroupStreaming: (groupId: string) => invoke<boolean>('is_group_streaming', { groupId }),
    getActiveGroupIds: () => invoke<string[]>('get_active_group_ids'),
  },
  system: {
    getEncoders: () => invoke<Encoders>('get_encoders'),
    testFfmpeg: () => invoke<string>('test_ffmpeg'),
  },
  settings: {
    get: () => invoke<AppSettings>('get_settings'),
    save: (settings: AppSettings) => invoke<void>('save_settings', { settings }),
    getProfilesPath: () => invoke<string>('get_profiles_path'),
    exportData: (exportPath: string) => invoke<void>('export_data', { exportPath }),
    clearData: () => invoke<void>('clear_data'),
  },
};
