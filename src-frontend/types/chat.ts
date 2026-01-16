import type { Platform } from './profile';

export interface ChatMessage {
  id: string;
  platform: Platform;
  username: string;
  message: string;
  timestamp: number;
}

// Chat platform enum
export type ChatPlatform = 'twitch' | 'tiktok' | 'youtube' | 'kick' | 'facebook';

// Chat connection status
export type ChatConnectionStatus = 'connected' | 'disconnected' | 'connecting' | 'error';

// Chat credentials (discriminated union based on platform)
export type ChatCredentials =
  | {
      type: 'twitch';
      channel: string;
      oauthToken?: string;
    }
  | {
      type: 'tiktok';
      username: string;
      sessionToken?: string;
    }
  | {
      type: 'youtube';
      channelId: string;
      apiKey?: string;
    }
  | {
      type: 'kick';
      channel: string;
    }
  | {
      type: 'facebook';
      pageId: string;
      accessToken?: string;
    };

// Chat configuration
export interface ChatConfig {
  platform: ChatPlatform;
  enabled: boolean;
  credentials: ChatCredentials;
}

// Platform status response
export interface ChatPlatformStatus {
  platform: ChatPlatform;
  status: ChatConnectionStatus;
  messageCount: number;
  error?: string;
}
