// Frontend-only factory functions + the platform-metadata table.
// Domain types live in @spiritstream/types — import them directly there.
// This file holds only runtime values: default-builders for new profiles /
// targets / settings, formatting helpers, and the platform display-name +
// default-URL map that the UI needs at runtime.

import type {
  Profile,
  ProfileSettings,
  RtmpInput,
  OutputGroup,
  StreamTarget,
  VideoSettings,
  AudioSettings,
  ContainerSettings,
  BackendSettings,
  ObsSettings,
  DiscordSettings,
  ChatSettings,
  OAuthAccount,
  OAuthSettings,
  Platform,
} from '@spiritstream/types';

import { PLATFORMS } from '@/types/generated-platforms';
export { PLATFORMS };

// ----------------------------------------------------------------------------
// Factory functions
// ----------------------------------------------------------------------------

export const createDefaultRtmpInput = (): RtmpInput => ({
  type: 'rtmp',
  bindAddress: '0.0.0.0',
  port: 1935,
  application: 'live',
  // Recomputed server-side on every save/load (RtmpInput::refresh_url);
  // the placeholder only exists until the first round-trip.
  url: 'rtmp://0.0.0.0:1935/live',
});

export const createDefaultVideoSettings = (): VideoSettings => ({
  codec: 'copy',
  width: 0,
  height: 0,
  fps: 0,
  bitrate: '0k',
  preset: null,
  profile: null,
  keyframeIntervalSeconds: null,
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
  enabled: true,
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
  enabled: true,
  video: createDefaultVideoSettings(),
  audio: createDefaultAudioSettings(),
  container: createDefaultContainerSettings(),
  streamTargets: [],
});

export const createDefaultStreamTarget = (service: Platform): StreamTarget => ({
  id: crypto.randomUUID(),
  service,
  name: PLATFORMS[service].displayName,
  url: PLATFORMS[service].defaultServer,
  streamKey: '',
  enabled: true,
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

export const createDefaultChatSettings = (): ChatSettings => ({
  twitchChannel: '',
  youtubeChannelId: '',
  trovoChannelId: '',
  kickChannel: '',
  tiktokUsername: '',
  facebookLiveVideoId: '',
  youtubeApiKey: '',
  twitchSendEnabled: false,
  youtubeSendEnabled: false,
  trovoSendEnabled: false,
  kickSendEnabled: false,
  sendAllEnabled: true,
  crosspostEnabled: false,
  followerOnlyDefault: false,
  youtubeUseApiKey: false,
  visiblePlatforms: [],
  visibilityPanelCollapsed: true,
});

export const createDefaultOAuthAccount = (): OAuthAccount => ({
  accessToken: '',
  refreshToken: '',
  expiresAt: 0,
  userId: '',
  username: '',
  displayName: '',
});

export const createDefaultOAuthSettings = (): OAuthSettings => ({
  twitch: createDefaultOAuthAccount(),
  youtube: createDefaultOAuthAccount(),
  kick: createDefaultOAuthAccount(),
  facebook: createDefaultOAuthAccount(),
});

export const createDefaultProfileSettings = (): ProfileSettings => ({
  themeId: 'spirit-dark',
  language: 'en',
  showNotifications: true,
  encryptStreamKeys: true,
  backend: createDefaultBackendSettings(),
  obs: createDefaultObsSettings(),
  discord: createDefaultDiscordSettings(),
  chat: createDefaultChatSettings(),
  oauth: createDefaultOAuthSettings(),
});

export const createDefaultProfile = (name: string = 'New Profile'): Profile => ({
  id: crypto.randomUUID(),
  name,
  encrypted: false,
  input: createDefaultRtmpInput(),
  outputGroups: [createPassthroughOutputGroup()],
  settings: createDefaultProfileSettings(),
  // Empty PII blocklist, strict matching by default.
  piiBlocklist: [],
  piiFuzzy: false,
  // Anonymous chat logging defaults ON for new profiles.
  // The salt is populated server-side on first save (see
  // ProfileManager::save_with_key_encryption → ensure_anonymous_salt).
  anonymousLogging: true,
  anonymousSalt: '',
});

// ----------------------------------------------------------------------------
// Formatting helpers
// ----------------------------------------------------------------------------

export const formatResolution = (video: VideoSettings): string => {
  if (video.codec === 'copy' || video.height === 0) {
    return 'Passthrough';
  }
  return `${video.height}p${video.fps}`;
};

