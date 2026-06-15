import type {
  ChatConfig,
  ChatLogStatus,
  ChatMessage,
  ChatPlatform,
  ChatPlatformStatus,
  ChatSendResult,
} from '@spiritstream/types';
import type { ChatConfigWire } from '../generated';
import {
  v1ChatConnectProxy,
  v1ChatSendProxy,
  v1ChatDisconnectProxy,
  v1ChatRetryProxy,
  v1ChatDisconnectAllProxy,
  v1ChatStatusProxy,
  v1ChatRecentMessagesProxy,
  v1ChatLogStatusProxy,
  v1ChatExportLogProxy,
  v1ChatSearchSessionProxy,
  v1ChatPlatformStatusProxy,
  v1ChatIsConnectedProxy,
  v1ChatReidentifyProxy,
} from '../generated';
import { confirmTokenHeader } from './_confirm';
import { parseChatMessages } from '../validation/chatMessage';

export const chat = {
  connect: async (config: ChatConfig): Promise<void> => {
    // ts-rs `ChatConfig` and utoipa `ChatConfigWire` serialize to identical
    // JSON (the handler does `wire.into()`); their TS types differ only in
    // enum representation, so the cast is sound at the wire boundary.
    await v1ChatConnectProxy({ body: config as unknown as ChatConfigWire, throwOnError: true });
  },
  /**
   * Connect a Facebook Live chat, gated by a one-shot `enable_facebook_chat`
   * confirm-token. The backend refuses Facebook payloads without
   * `X-Confirm-Token` for that intent — connecting Facebook reveals the
   * streamer's real-name account per Meta's Name Policy, so the
   * warning-acknowledge → connect dance must be deliberate.
   */
  connectFacebook: async (config: ChatConfig): Promise<void> => {
    await v1ChatConnectProxy({
      headers: await confirmTokenHeader('enable_facebook_chat'),
      body: config as unknown as ChatConfigWire,
      throwOnError: true,
    });
  },
  sendMessage: async (
    message: string,
    targetPlatforms?: ChatPlatform[]
  ): Promise<ChatSendResult[]> => {
    const { data } = await v1ChatSendProxy({
      body: targetPlatforms === undefined ? { message } : { message, targetPlatforms },
      throwOnError: true,
    });
    return data as ChatSendResult[];
  },
  disconnect: async (platform: ChatPlatform): Promise<void> => {
    await v1ChatDisconnectProxy({ path: { platform }, throwOnError: true });
  },
  retryConnection: async (platform: ChatPlatform): Promise<void> => {
    await v1ChatRetryProxy({ path: { platform }, throwOnError: true });
  },
  /**
   * Connect a single platform on demand (the per-platform Connect button),
   * independent of streaming. The backend builds credentials from the active
   * profile, so the frontend passes no creds. Backed by the same endpoint as
   * {@link retryConnection}; clears the platform's deliberate-disconnect
   * intent server-side on success.
   */
  connectPlatform: async (platform: ChatPlatform): Promise<void> => {
    await v1ChatRetryProxy({ path: { platform }, throwOnError: true });
  },
  disconnectAll: async (): Promise<void> => {
    await v1ChatDisconnectAllProxy({ throwOnError: true });
  },
  getStatus: async (): Promise<ChatPlatformStatus[]> => {
    const { data } = await v1ChatStatusProxy({ throwOnError: true });
    return data as ChatPlatformStatus[];
  },
  /**
   * Recent messages from the backend's in-memory ring — the server-side
   * replay source fetched on page load / WS reconnect to repopulate chat.
   * Untrusted inbound: validated at runtime with `parseChatMessages` (Zod)
   * before reaching the UI, so a malformed/hostile payload can't slip
   * through the compile-time-only generated types.
   */
  getRecentMessages: async (): Promise<ChatMessage[]> => {
    const { data } = await v1ChatRecentMessagesProxy({ throwOnError: true });
    return parseChatMessages(data);
  },
  getLogStatus: async (): Promise<ChatLogStatus> => {
    const { data } = await v1ChatLogStatusProxy({ throwOnError: true });
    return data as ChatLogStatus;
  },
  exportLog: async (path: string): Promise<void> => {
    await v1ChatExportLogProxy({ body: { path }, throwOnError: true });
  },
  searchSession: async (query: string, limit?: number): Promise<ChatMessage[]> => {
    const { data } = await v1ChatSearchSessionProxy({ body: { query, limit }, throwOnError: true });
    return parseChatMessages(data);
  },
  getPlatformStatus: async (platform: ChatPlatform): Promise<ChatPlatformStatus | null> => {
    const { data } = await v1ChatPlatformStatusProxy({ path: { platform }, throwOnError: true });
    return (data ?? null) as ChatPlatformStatus | null;
  },
  isConnected: async (): Promise<boolean> => {
    const { data } = await v1ChatIsConnectedProxy({ throwOnError: true });
    return data.connected;
  },
  /**
   * Re-identify a pseudonymised author: confirm whether `candidate` (a real
   * name the user already suspects) matches the `pseudonym` carried on a
   * message's `author.login`/`author.userId`. The salt stays server-side and
   * the hash is one-way, so this only CONFIRMS a guess — it can't reverse it.
   */
  reidentify: async (candidate: string, pseudonym: string): Promise<boolean> => {
    const { data } = await v1ChatReidentifyProxy({
      body: { candidate, pseudonym },
      throwOnError: true,
    });
    return data.matches;
  },
};
