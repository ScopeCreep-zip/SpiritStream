import type { OutputGroup, Profile } from '@spiritstream/types';
import { fetchTypedJson } from './_internal';

export const stream = {
  /** Start streaming for a single output group. Returns the FFmpeg process PID */
  start: async (group: OutputGroup, incomingUrl: string) => {
    const { pid } = await fetchTypedJson<{ pid: number }>(
      'POST',
      `/api/v1/streams/groups/${encodeURIComponent(group.id)}`,
      undefined,
      { group, incomingUrl }
    );
    return pid;
  },
  /**
   * Start streaming. Send EVERY group — eligibility (group enabled +
   * at least one enabled target) is decided server-side. Returns the
   * authoritative list of started group ids alongside the PIDs.
   */
  startAll: async (groups: OutputGroup[], incomingUrl: string) => {
    return fetchTypedJson<{ pids: number[]; startedGroupIds: string[] }>(
      'POST',
      '/api/v1/streams',
      undefined,
      { groups, incomingUrl }
    );
  },
  /** Stop streaming for a specific output group */
  stop: async (groupId: string) => {
    await fetchTypedJson<{ stopped: boolean }>(
      'DELETE',
      `/api/v1/streams/groups/${encodeURIComponent(groupId)}`
    );
  },
  /** Stop all active streams */
  stopAll: async () => {
    await fetchTypedJson<{ stopped: boolean }>('DELETE', '/api/v1/streams');
  },
  getActiveCount: async () => {
    const status = await fetchTypedJson<{ activeCount: number; activeGroupIds: string[] }>(
      'GET',
      '/api/v1/streams'
    );
    return status.activeCount;
  },
  getActiveGroupIds: async () => {
    const status = await fetchTypedJson<{ activeGroupIds: string[] }>('GET', '/api/v1/streams');
    return status.activeGroupIds;
  },
  toggleTarget: async (
    targetId: string,
    enabled: boolean,
    group: OutputGroup,
    incomingUrl: string
  ) => {
    const { pid } = await fetchTypedJson<{ pid: number }>(
      'PATCH',
      `/api/v1/streams/targets/${encodeURIComponent(targetId)}`,
      undefined,
      { enabled, group, incomingUrl }
    );
    return pid;
  },
  isTargetDisabled: (targetId: string) =>
    fetchTypedJson<{ disabled: boolean }>(
      'GET',
      `/api/v1/streams/targets/${encodeURIComponent(targetId)}/disabled`
    ).then((r) => r.disabled),
  /** Retry a failed stream. Returns PID and next delay if another retry is needed */
  retry: (groupId: string) =>
    fetchTypedJson<{ pid: number; nextDelaySecs: number | null }>(
      'POST',
      `/api/v1/streams/groups/${encodeURIComponent(groupId)}/retry`
    ),
  /**
   * Validate an entire profile's encoding config server-side. Throws on
   * failure with `err.kind === 'invalid_stream_config'` and `err.details.reasons`
   * carrying every `ValidationIssue`. Used for decorative live feedback in
   * modals; the same check runs inside `stream.start`.
   */
  validate: (profile: Profile) =>
    fetchTypedJson<{ valid: boolean }>('POST', '/api/v1/streams/validate', undefined, {
      profile,
    }),
};
