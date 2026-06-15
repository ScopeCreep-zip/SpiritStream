import type { OutputGroup, Profile } from '@spiritstream/types';
import {
  v1StreamsStart,
  v1StreamsStartAll,
  v1StreamsStop,
  v1StreamsStopAll,
  v1StreamsStatus,
  v1StreamsToggleTarget,
  v1StreamTargetDisabledProxy,
  v1StreamsRetry,
  v1StreamsValidate,
} from '../generated';

export const stream = {
  /** Start streaming for a single output group. Returns the FFmpeg process PID */
  start: async (group: OutputGroup, incomingUrl: string): Promise<number> => {
    const { data } = await v1StreamsStart({
      path: { group_id: group.id },
      body: { group, incomingUrl },
      throwOnError: true,
    });
    return data.pid;
  },
  /**
   * Start streaming. Send EVERY group — eligibility (group enabled +
   * at least one enabled target) is decided server-side. Returns the
   * authoritative list of started group ids alongside the PIDs.
   */
  startAll: async (
    groups: OutputGroup[],
    incomingUrl: string
  ): Promise<{ pids: number[]; startedGroupIds: string[] }> => {
    const { data } = await v1StreamsStartAll({ body: { groups, incomingUrl }, throwOnError: true });
    return data;
  },
  /** Stop streaming for a specific output group */
  stop: async (groupId: string): Promise<void> => {
    await v1StreamsStop({ path: { group_id: groupId }, throwOnError: true });
  },
  /** Stop all active streams */
  stopAll: async (): Promise<void> => {
    await v1StreamsStopAll({ throwOnError: true });
  },
  getActiveCount: async (): Promise<number> => {
    const { data } = await v1StreamsStatus({ throwOnError: true });
    return data.activeCount;
  },
  getActiveGroupIds: async (): Promise<string[]> => {
    const { data } = await v1StreamsStatus({ throwOnError: true });
    return data.activeGroupIds;
  },
  toggleTarget: async (
    targetId: string,
    enabled: boolean,
    group: OutputGroup,
    incomingUrl: string
  ): Promise<number> => {
    const { data } = await v1StreamsToggleTarget({
      path: { target_id: targetId },
      body: { enabled, group, incomingUrl },
      throwOnError: true,
    });
    return data.pid;
  },
  isTargetDisabled: async (targetId: string): Promise<boolean> => {
    const { data } = await v1StreamTargetDisabledProxy({
      path: { target_id: targetId },
      throwOnError: true,
    });
    return data.disabled;
  },
  /** Retry a failed stream. Returns PID and next delay if another retry is needed */
  retry: async (groupId: string): Promise<{ pid: number; nextDelaySecs: number | null }> => {
    const { data } = await v1StreamsRetry({ path: { group_id: groupId }, throwOnError: true });
    return { pid: data.pid, nextDelaySecs: data.nextDelaySecs ?? null };
  },
  /**
   * Validate an entire profile's encoding config server-side. Throws on
   * failure with `err.kind === 'invalid_stream_config'` and `err.details.reasons`
   * carrying every `ValidationIssue`. Used for decorative live feedback in
   * modals; the same check runs inside `stream.start`.
   */
  validate: async (profile: Profile): Promise<{ valid: boolean }> => {
    const { data } = await v1StreamsValidate({ body: { profile }, throwOnError: true });
    return data;
  },
};
