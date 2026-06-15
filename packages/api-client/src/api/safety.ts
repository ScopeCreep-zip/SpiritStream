import { v1SafetyPanic } from '../generated';

export const safety = {
  /**
   * Trigger panic disconnect: stops every active stream, disconnects
   * chat + OBS, wipes in-memory secret caches, records an audit
   * entry. Returns the action summary.
   */
  panic: async (): Promise<{ streamsStopped: number; elapsedMs: number }> => {
    const { data } = await v1SafetyPanic({ throwOnError: true });
    return data;
  },
};
