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

// Twitch authentication options
export type TwitchAuth =
  | {
      method: 'userToken';
      oauthToken: string;
    }
  | {
      method: 'appOAuth';
      accessToken: string;
      refreshToken?: string;
      expiresAt?: number;
    };

// YouTube authentication options
export type YouTubeAuth =
  | {
      method: 'apiKey';
      key: string;
    }
  | {
      method: 'appOAuth';
      accessToken: string;
      refreshToken?: string;
      expiresAt?: number;
    };

// Chat credentials (discriminated union based on platform)
export type ChatCredentials =
  | {
      type: 'twitch';
      channel: string;
      auth?: TwitchAuth;
    }
  | {
      type: 'tiktok';
      username: string;
      sessionToken?: string;
    }
  | {
      type: 'youtube';
      channelId: string;
      auth: YouTubeAuth;
    }
  | {
      type: 'kick';
      channel: string;
    }
  | {
      type: 'facebook';
      videoId: string;
      accessToken: string;
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

// OAuth account info returned from oauth_get_account
export interface OAuthAccount {
  loggedIn: boolean;
  userId?: string;
  username?: string;
  displayName?: string;
}

// OAuth flow result returned from oauth_start_flow
export interface OAuthFlowResult {
  authUrl: string;
  callbackPort: number;
  state: string;
}
