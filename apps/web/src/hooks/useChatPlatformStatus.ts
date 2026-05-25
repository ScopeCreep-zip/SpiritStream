import { useEffect, useState } from 'react';
import { api } from '@/lib/client';
import { clientConfig } from '@/lib/constants';
import { logger } from '@/lib/logger';
import type { ChatPlatformStatus } from '@spiritstream/types';

/**
 * Per-platform chat connection status + active stream count, refreshed
 * on a polling interval (`clientConfig.CHAT_POLL_INTERVAL_MS`). Single
 * source of truth so multiple components in the chat surface don't each
 * spin up their own poller.
 */
export interface ChatPlatformStatusState {
  statuses: ChatPlatformStatus[];
  activeStreamCount: number;
}

export function useChatPlatformStatus(): ChatPlatformStatusState {
  const [statuses, setStatuses] = useState<ChatPlatformStatus[]>([]);
  const [activeStreamCount, setActiveStreamCount] = useState(0);

  // Initial load + repeating poll. Two separate effects so the
  // first load isn't gated on the interval tick.
  useEffect(() => {
    const load = async (): Promise<void> => {
      try {
        const [loadedStatuses, streamCount] = await Promise.all([
          api.chat.getStatus(),
          api.stream.getActiveCount(),
        ]);
        setStatuses(loadedStatuses);
        setActiveStreamCount(streamCount);
      } catch (error) {
        logger.error('Failed to load chat status:', error);
      }
    };
    load();
  }, []);

  useEffect(() => {
    const interval = setInterval(async () => {
      try {
        const [loadedStatuses, streamCount] = await Promise.all([
          api.chat.getStatus(),
          api.stream.getActiveCount(),
        ]);
        setStatuses(loadedStatuses);
        setActiveStreamCount(streamCount);
      } catch (error) {
        logger.error('Failed to refresh chat status:', error);
      }
    }, clientConfig.CHAT_POLL_INTERVAL_MS);

    return () => clearInterval(interval);
  }, []);

  return { statuses, activeStreamCount };
}
