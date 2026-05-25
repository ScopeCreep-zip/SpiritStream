import type {
  ChatConfig,
  ChatLogStatus,
  ChatMessage,
  ChatPlatform,
  ChatPlatformStatus,
  ChatSendResult,
} from '@spiritstream/types';
import { fetchTypedJson } from './_internal';

export const chat = {
  connect: async (config: ChatConfig) => {
    await fetchTypedJson<unknown>('POST', '/api/v1/chat/connections', undefined, { config });
  },
  sendMessage: (message: string) =>
    fetchTypedJson<ChatSendResult[]>('POST', '/api/v1/chat/messages', undefined, { message }),
  disconnect: async (platform: ChatPlatform) => {
    await fetchTypedJson<unknown>(
      'DELETE',
      `/api/v1/chat/connections/${encodeURIComponent(String(platform))}`,
    );
  },
  retryConnection: async (platform: ChatPlatform) => {
    await fetchTypedJson<unknown>(
      'POST',
      `/api/v1/chat/connections/${encodeURIComponent(String(platform))}/retry`,
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
      `/api/v1/chat/connections/${encodeURIComponent(String(platform))}`,
    ),
  isConnected: () =>
    fetchTypedJson<{ connected: boolean }>('GET', '/api/v1/chat/connected').then((r) => r.connected),
};
