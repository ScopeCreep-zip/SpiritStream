import type {
  ChatConfig,
  ChatLogStatus,
  ChatMessage,
  ChatPlatform,
  ChatPlatformStatus,
  ChatSendResult,
} from '@spiritstream/types';
import { fetchTypedJson, withConfirmToken } from './_internal';

export const chat = {
  connect: async (config: ChatConfig) => {
    await fetchTypedJson<unknown>('POST', '/api/v1/chat/connections', undefined, { config });
  },
  /**
   * Connect a Facebook Live chat, gated by a one-shot `enable_facebook_chat`
   * confirm-token. The backend's POST /api/v1/chat/connections handler
   * refuses Facebook payloads without `X-Confirm-Token` for that intent —
   * the gate exists because connecting Facebook reveals the streamer's
   * real-name account per Meta's Name Policy, and the warning-acknowledge
   * → connect dance must be deliberate (no muscle-memory click-through).
   */
  connectFacebook: async (config: ChatConfig) =>
    withConfirmToken('enable_facebook_chat', (headers) =>
      fetchTypedJson<unknown>('POST', '/api/v1/chat/connections', undefined, { config }, headers)
    ),
  sendMessage: (message: string, targetPlatforms?: ChatPlatform[]) =>
    fetchTypedJson<ChatSendResult[]>(
      'POST',
      '/api/v1/chat/messages',
      undefined,
      targetPlatforms === undefined ? { message } : { message, targetPlatforms }
    ),
  disconnect: async (platform: ChatPlatform) => {
    await fetchTypedJson<unknown>(
      'DELETE',
      `/api/v1/chat/connections/${encodeURIComponent(String(platform))}`
    );
  },
  retryConnection: async (platform: ChatPlatform) => {
    await fetchTypedJson<unknown>(
      'POST',
      `/api/v1/chat/connections/${encodeURIComponent(String(platform))}/retry`
    );
  },
  disconnectAll: async () => {
    await fetchTypedJson<unknown>('DELETE', '/api/v1/chat/connections');
  },
  getStatus: () => fetchTypedJson<ChatPlatformStatus[]>('GET', '/api/v1/chat/connections'),
  getLogStatus: () => fetchTypedJson<ChatLogStatus>('GET', '/api/v1/chat/log'),
  exportLog: async (path: string) => {
    await fetchTypedJson<unknown>('POST', '/api/v1/chat/log/export', undefined, { path });
  },
  searchSession: (query: string, limit?: number) =>
    fetchTypedJson<ChatMessage[]>('POST', '/api/v1/chat/log/search', undefined, { query, limit }),
  getPlatformStatus: (platform: ChatPlatform) =>
    fetchTypedJson<ChatPlatformStatus | null>(
      'GET',
      `/api/v1/chat/connections/${encodeURIComponent(String(platform))}`
    ),
  isConnected: () =>
    fetchTypedJson<{ connected: boolean }>('GET', '/api/v1/chat/connected').then(
      (r) => r.connected
    ),
};
