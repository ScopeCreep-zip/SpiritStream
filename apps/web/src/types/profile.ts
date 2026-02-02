/**
 * Platform/service types for stream targets
 * Auto-generated from data/streaming-platforms.json
 */
import type { Platform } from './generated-platforms';
import type { ObsIntegrationDirection } from './api';
export type { Platform };

// ============================================================================
// Profile Settings Types (per-profile configuration)
// ============================================================================

/**
 * Backend/Remote access settings for a profile
 */
export interface BackendSettings {
  remoteEnabled: boolean;
  uiEnabled: boolean;
  host: string;
  port: number;
  token: string;
}

/**
 * OBS WebSocket integration settings for a profile
 */
export interface ObsSettings {
  host: string;
  port: number;
  password: string;
  useAuth: boolean;
  direction: ObsIntegrationDirection;
  autoConnect: boolean;
}

/**
 * Discord webhook integration settings for a profile
 */
export interface DiscordSettings {
  webhookEnabled: boolean;
  webhookUrl: string;
  goLiveMessage: string;
  cooldownEnabled: boolean;
  cooldownSeconds: number;
  imagePath: string;
}

/**
 * Per-profile settings (theme, language, integrations, security)
 */
export interface ProfileSettings {
  // UI Settings
  themeId: string;
  language: string;
  showNotifications: boolean;

  // Security Settings
  encryptStreamKeys: boolean;

  // Integration Settings
  backend: BackendSettings;
  obs: ObsSettings;
  discord: DiscordSettings;
}

/**
 * RTMP Input configuration - where the stream enters the system
 */
export interface RtmpInput {
  type: 'rtmp';
  bindAddress: string; // e.g., "0.0.0.0"
  port: number; // e.g., 1935
  application: string; // e.g., "live"
}

/**
 * Video encoding settings
 */
export interface VideoSettings {
  codec: string; // e.g., "libx264", "h264_nvenc"
  width: number; // e.g., 1920
  height: number; // e.g., 1080
  fps: number; // e.g., 60
  bitrate: string; // e.g., "6000k"
  preset?: string; // e.g., "veryfast", "p4"
  profile?: string; // e.g., "high", "main"
  keyframeIntervalSeconds?: number; // e.g., 2
}

/**
 * Audio encoding settings
 */
export interface AudioSettings {
  codec: string; // e.g., "aac"
  bitrate: string; // e.g., "160k"
  channels: number; // e.g., 2
  sampleRate: number; // e.g., 48000
}

/**
 * Container/muxing settings
 */
export interface ContainerSettings {
  format: string; // e.g., "flv"
}

/**
 * Stream target - RTMP destination
 */
export interface StreamTarget {
  id: string;
  service: Platform; // renamed from 'platform'
  name: string;
  url: string;
  streamKey: string; // supports ${ENV_VAR} syntax
}

/**
 * Output group - encoding profile with stream targets
 */
export interface OutputGroup {
  id: string;
  name: string;
  isDefault?: boolean; // True for the immutable passthrough group
  generatePts?: boolean; // Generate PTS timestamps and sync audio (default: true)
  video: VideoSettings;
  audio: AudioSettings;
  container: ContainerSettings;
  streamTargets: StreamTarget[];
}

/**
 * Profile - top-level configuration entity
 */
export interface Profile {
  id: string;
  name: string;
  encrypted: boolean;
  input: RtmpInput;
  outputGroups: OutputGroup[];
  /** Per-profile settings (theme, integrations, security) */
  settings: ProfileSettings;
}

/**
 * Profile summary for list display
 */
export interface ProfileSummary {
  id: string;
  name: string;
  resolution: string;
  bitrate: number;
  targetCount: number;
  services: Platform[]; // NEW: list of configured services
  isEncrypted?: boolean;
}

/**
 * Platform configuration constants
 * Auto-generated from data/streaming-platforms.json
 */
import { PLATFORMS } from './generated-platforms';
export { PLATFORMS };

/**
 * Factory functions for creating default objects
 */
export const createDefaultRtmpInput = (): RtmpInput => ({
  type: 'rtmp',
  bindAddress: '0.0.0.0',
  port: 1935,
  application: 'live',
});

export const createDefaultVideoSettings = (): VideoSettings => ({
  codec: 'copy',
  width: 0,
  height: 0,
  fps: 0,
  bitrate: '0k',
  preset: undefined,
  profile: undefined,
  keyframeIntervalSeconds: undefined,
});

export const createDefaultAudioSettings = (): AudioSettings => ({
  codec: 'copy',
  bitrate: '0k',
  channels: 0,
  sampleRate: 0,
});

export const createDefaultContainerSettings = (): ContainerSettings => ({
  format: 'flv',
});

export const createDefaultOutputGroup = (): OutputGroup => ({
  id: crypto.randomUUID(),
  name: 'New Output Group',
  isDefault: false,
  generatePts: true,
  video: createDefaultVideoSettings(),
  audio: createDefaultAudioSettings(),
  container: createDefaultContainerSettings(),
  streamTargets: [],
});

export const createPassthroughOutputGroup = (): OutputGroup => ({
  id: 'default',
  name: 'Passthrough (Default)',
  isDefault: true,
  generatePts: true,
  video: createDefaultVideoSettings(), // codec: 'copy'
  audio: createDefaultAudioSettings(), // codec: 'copy'
  container: createDefaultContainerSettings(),
  streamTargets: [],
});

export const createDefaultStreamTarget = (service: Platform): StreamTarget => ({
  id: crypto.randomUUID(),
  service,
  name: PLATFORMS[service].displayName,
  url: PLATFORMS[service].defaultServer,
  streamKey: '',
});

export const createDefaultBackendSettings = (): BackendSettings => ({
  remoteEnabled: false,
  uiEnabled: false,
  host: '127.0.0.1',
  port: 8008,
  token: '',
});

export const createDefaultObsSettings = (): ObsSettings => ({
  host: 'localhost',
  port: 4455,
  password: '',
  useAuth: false,
  direction: 'disabled',
  autoConnect: false,
});

export const createDefaultDiscordSettings = (): DiscordSettings => ({
  webhookEnabled: false,
  webhookUrl: '',
  goLiveMessage: '**Stream is now live!** 🎮\n\nCome join the stream!',
  cooldownEnabled: true,
  cooldownSeconds: 60,
  imagePath: '',
});

export const createDefaultProfileSettings = (): ProfileSettings => ({
  themeId: 'spirit-dark',
  language: 'en',
  showNotifications: true,
  encryptStreamKeys: true,
  backend: createDefaultBackendSettings(),
  obs: createDefaultObsSettings(),
  discord: createDefaultDiscordSettings(),
});

export const createDefaultProfile = (name: string = 'New Profile'): Profile => ({
  id: crypto.randomUUID(),
  name,
  encrypted: false,
  input: createDefaultRtmpInput(),
  outputGroups: [createPassthroughOutputGroup()], // Always include default passthrough group
  settings: createDefaultProfileSettings(),
});

/**
 * Helper to format resolution string from video settings
 */
export const formatResolution = (video: VideoSettings): string => {
  // Check if this is copy/passthrough mode
  if (video.codec === 'copy' || video.height === 0) {
    return 'Passthrough';
  }
  const height = video.height;
  const fps = video.fps;
  return `${height}p${fps}`;
};

/**
 * Helper to parse bitrate string to number (in kbps)
 */
export const parseBitrateToKbps = (bitrate: string): number => {
  const match = bitrate.match(/^(\d+)(k|m|K|M)?$/i);
  if (!match) return 0;
  const value = parseInt(match[1], 10);
  const unit = match[2]?.toLowerCase();
  if (unit === 'm') return value * 1000;
  return value;
};
