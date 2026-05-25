import { fetchTypedJson } from './_internal';

export const safety = {
  /**
   * Trigger panic disconnect: stops every active stream, disconnects
   * chat + OBS, wipes in-memory secret caches, records an audit
   * entry. Returns the action summary.
   */
  panic: () =>
    fetchTypedJson<{ streamsStopped: number; elapsedMs: number }>(
      'POST',
      '/api/v1/safety/panic',
    ),
};
