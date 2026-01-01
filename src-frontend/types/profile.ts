/**
 * Platform types for stream targets
 */
export type Platform = 'youtube' | 'twitch' | 'kick' | 'facebook' | 'custom';

/**
 * Stream target - RTMP destination
 */
export interface StreamTarget {
  id: string;
  platform: Platform;
  name: string;
  url: string;
  streamKey: string;
  port: number;
}

/**
 * Output group - encoding profile with stream targets
 */
export interface OutputGroup {
  id: string;
  name?: string;
  videoEncoder: string;
  resolution: string;
  videoBitrate: number;
  fps: number;
  audioCodec: string;
  audioBitrate: number;
  generatePts: boolean;
  streamTargets: StreamTarget[];
}

/**
 * Theme customization
 */
export interface Theme {
  name: string;
  primaryColor?: string;
  accentColor?: string;
}

/**
 * Profile - top-level configuration entity
 */
export interface Profile {
  id: string;
  name: string;
  incomingUrl: string;
  outputGroups: OutputGroup[];
  theme?: Theme;
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
}

/**
 * Platform configuration constants
 */
export const PLATFORMS: Record<Platform, {
  name: string;
  abbreviation: string;
  color: string;
  textColor: string;
  defaultServer: string;
}> = {
  youtube: {
    name: 'YouTube',
    abbreviation: 'YT',
    color: '#FF0000',
    textColor: '#FFFFFF',
    defaultServer: 'rtmp://a.rtmp.youtube.com/live2',
  },
  twitch: {
    name: 'Twitch',
    abbreviation: 'TW',
    color: '#9146FF',
    textColor: '#FFFFFF',
    defaultServer: 'rtmp://live.twitch.tv/app',
  },
  kick: {
    name: 'Kick',
    abbreviation: 'K',
    color: '#53FC18',
    textColor: '#000000',
    defaultServer: 'rtmps://fa723fc1b171.global-contribute.live-video.net/app',
  },
  facebook: {
    name: 'Facebook Live',
    abbreviation: 'FB',
    color: '#1877F2',
    textColor: '#FFFFFF',
    defaultServer: 'rtmps://live-api-s.facebook.com:443/rtmp',
  },
  custom: {
    name: 'Custom RTMP',
    abbreviation: 'RT',
    color: 'var(--primary)',
    textColor: '#FFFFFF',
    defaultServer: '',
  },
};
