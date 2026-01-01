import { invoke } from '@tauri-apps/api/core';
import type { Profile, OutputGroup, StreamTarget } from '@/types/profile';
import type { Encoders, StreamInfo } from '@/types/stream';
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
  },
  stream: {
    start: (group: OutputGroup, incomingUrl: string) =>
      invoke<StreamInfo>('start_stream', { group, incomingUrl }),
    stop: (groupId: string) => invoke<void>('stop_stream', { groupId }),
    stopAll: () => invoke<void>('stop_all_streams'),
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
    exportData: () => invoke<string>('export_data'),
    clearData: () => invoke<void>('clear_data'),
  },
};
