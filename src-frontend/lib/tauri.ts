import { invoke } from '@tauri-apps/api/core';
import type { Profile, OutputGroup, StreamTarget } from '@/types/profile';
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
    create: (name: string) => invoke<Profile>('create_profile', { name }),
    createStreamTarget: (url: string, streamKey: string) =>
      invoke<StreamTarget>('create_stream_target', { url, streamKey }),
  },
  stream: {
    start: (group: OutputGroup, incomingUrl: string) =>
      invoke<number>('start_stream', { group, incomingUrl }),
    startSimple: (group: OutputGroup, incomingUrl: string) =>
      invoke<number>('start_stream_simple', { group, incomingUrl }),
    stop: (groupId: string) => invoke<void>('stop_stream', { groupId }),
    stopAll: () => invoke<void>('stop_all_streams'),
    getActiveCount: () => invoke<number>('get_active_stream_count'),
    isGroupStreaming: (groupId: string) => invoke<boolean>('is_group_streaming', { groupId }),
    getActiveGroupIds: () => invoke<string[]>('get_active_group_ids'),
  },
  target: {
    add: (groupId: string, target: StreamTarget) =>
      invoke<void>('add_stream_target', { groupId, target }),
    update: (groupId: string, targetId: string, target: Partial<StreamTarget>) =>
      invoke<void>('update_stream_target', { groupId, targetId, target }),
    remove: (groupId: string, targetId: string) =>
      invoke<void>('remove_stream_target', { groupId, targetId }),
  },
  system: {
    getEncoders: () => invoke<Encoders>('get_encoders'),
    testFfmpeg: () => invoke<string>('test_ffmpeg'),
    greet: (name: string) => invoke<string>('greet', { name }),
  },
  settings: {
    get: () => invoke<AppSettings>('get_settings'),
    save: (settings: AppSettings) => invoke<void>('save_settings', { settings }),
    getProfilesPath: () => invoke<string>('get_profiles_path'),
    exportData: (exportPath: string) => invoke<void>('export_data', { exportPath }),
    clearData: () => invoke<void>('clear_data'),
  },
};
