/**
 * Platform/service types for stream targets
 * Auto-generated from data/streaming-platforms.json
 */
import type { Platform } from './generated-platforms';
export type { Platform };

// Import and re-export source and scene types
import type { Source, SourceType } from './source';
import type { Scene, SourceLayer, Transform, Crop, AudioMixer, AudioTrack, SceneTransition, TransitionType } from './scene';
export type { Source, SourceType };
export type { Scene, SourceLayer, Transform, Crop, AudioMixer, AudioTrack, SceneTransition, TransitionType };

// Import factory functions for creating default sources/scenes
import { createDefaultRtmpSource } from './source';
import { createDefaultScene, addFullscreenLayerToScene } from './scene';

/**
 * RTMP Input configuration - where the stream enters the system
 * LEGACY: Kept for backward compatibility, use Sources instead
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
  /** LEGACY: RTMP input configuration (for backward compatibility) */
  input?: RtmpInput;
  /** NEW: Multiple input sources (RTMP, cameras, screens, files, etc.) */
  sources: Source[];
  /** NEW: Scene compositions (layouts with positioned sources) */
  scenes: Scene[];
  /** NEW: Currently active scene ID */
  activeSceneId?: string;
  /** NEW: Default transition for scene switching */
  defaultTransition?: SceneTransition;
  outputGroups: OutputGroup[];
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
  video: createDefaultVideoSettings(),
  audio: createDefaultAudioSettings(),
  container: createDefaultContainerSettings(),
  streamTargets: [],
});

export const createPassthroughOutputGroup = (): OutputGroup => ({
  id: 'default',
  name: 'Passthrough (Default)',
  isDefault: true,
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

export const createDefaultProfile = (name: string = 'New Profile'): Profile => {
  // Create default RTMP source
  const rtmpSource = createDefaultRtmpSource('Main Input');

  // Create default scene with the RTMP source
  const defaultScene = addFullscreenLayerToScene(
    createDefaultScene('Main'),
    rtmpSource.id
  );

  return {
    id: crypto.randomUUID(),
    name,
    encrypted: false,
    // Legacy input field is now optional
    input: undefined,
    // New multi-source fields
    sources: [rtmpSource],
    scenes: [defaultScene],
    activeSceneId: defaultScene.id,
    outputGroups: [createPassthroughOutputGroup()],
  };
};

/**
 * Create a profile with legacy input format (for backward compatibility)
 */
export const createLegacyProfile = (name: string = 'New Profile'): Profile => ({
  id: crypto.randomUUID(),
  name,
  encrypted: false,
  input: createDefaultRtmpInput(),
  sources: [],
  scenes: [],
  activeSceneId: undefined,
  outputGroups: [createPassthroughOutputGroup()],
});

/**
 * Migrate a legacy profile to the new multi-source format
 * Call this after loading a profile to ensure it uses the new format
 */
export const migrateProfileIfNeeded = (profile: Profile): Profile => {
  // Skip if already migrated (has sources)
  if (profile.sources.length > 0) {
    return profile;
  }

  // Check if legacy input exists
  if (!profile.input) {
    // No input at all - create default source and scene
    const rtmpSource = createDefaultRtmpSource('Main Input');
    const defaultScene = addFullscreenLayerToScene(
      createDefaultScene('Main'),
      rtmpSource.id
    );

    return {
      ...profile,
      input: undefined,
      sources: [rtmpSource],
      scenes: [defaultScene],
      activeSceneId: defaultScene.id,
    };
  }

  // Migrate legacy input to source
  const rtmpSource = {
    type: 'rtmp' as const,
    id: crypto.randomUUID(),
    name: 'Main Input',
    bindAddress: profile.input.bindAddress,
    port: profile.input.port,
    application: profile.input.application,
  };

  const defaultScene = addFullscreenLayerToScene(
    createDefaultScene('Main'),
    rtmpSource.id
  );

  return {
    ...profile,
    input: undefined, // Clear legacy field
    sources: [rtmpSource],
    scenes: [defaultScene],
    activeSceneId: defaultScene.id,
  };
};

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

/**
 * Get the incoming RTMP URL from a profile
 * Checks both legacy input and new sources
 */
export const getIncomingUrl = (profile: Profile): string | null => {
  // First check legacy input field
  if (profile.input) {
    return `rtmp://${profile.input.bindAddress}:${profile.input.port}/${profile.input.application}`;
  }

  // Check new sources - find first RTMP source
  for (const source of profile.sources) {
    if (source.type === 'rtmp') {
      return `rtmp://${source.bindAddress}:${source.port}/${source.application}`;
    }
  }

  return null;
};

/**
 * Get the RTMP input configuration from a profile
 * Returns either legacy input or first RTMP source as RtmpInput format
 */
export const getRtmpInput = (profile: Profile): RtmpInput | null => {
  // First check legacy input field
  if (profile.input) {
    return profile.input;
  }

  // Check new sources - find first RTMP source
  for (const source of profile.sources) {
    if (source.type === 'rtmp') {
      return {
        type: 'rtmp',
        bindAddress: source.bindAddress,
        port: source.port,
        application: source.application,
      };
    }
  }

  return null;
};
