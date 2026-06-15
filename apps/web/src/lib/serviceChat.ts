import type { Platform, ChatPlatform, ChatSettings } from '@spiritstream/types';

/**
 * Stream-target service → the chat platform whose settings can be set up for
 * it, or `null` when the service has no chat integration. The set mirrors the
 * platforms with a setup card in `ChatPanel` (twitch, youtube, trovo, kick,
 * tiktok, facebook) — i.e. "chat is available for setup". Service strings are
 * exact `Platform` union members.
 */
const SERVICE_TO_CHAT: Partial<Record<Platform, ChatPlatform>> = {
  Twitch: 'twitch',
  'YouTube - RTMPS': 'youtube',
  Kick: 'kick',
  Trovo: 'trovo',
  'TikTok Live': 'tiktok',
  'Facebook Live': 'facebook',
};

export function serviceToChatPlatform(service: Platform): ChatPlatform | null {
  return SERVICE_TO_CHAT[service] ?? null;
}

/**
 * Whether a chat platform is "set up" on the active profile — i.e. its
 * channel / username / video-id is filled in. Mirrors ChatPanel's
 * auto-visibility check. Gates the per-row chat connect/disconnect toggle.
 */
export function isChatPlatformConfigured(platform: ChatPlatform, chat: ChatSettings): boolean {
  switch (platform) {
    case 'twitch':
      return chat.twitchChannel.trim() !== '';
    case 'youtube':
      return chat.youtubeChannelId.trim() !== '';
    case 'trovo':
      return chat.trovoChannelId.trim() !== '';
    case 'kick':
      return chat.kickChannel.trim() !== '';
    case 'tiktok':
      return chat.tiktokUsername.trim() !== '';
    case 'facebook':
      return (chat.facebookLiveVideoId ?? '').trim() !== '';
  }
}
