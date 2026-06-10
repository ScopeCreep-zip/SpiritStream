import type { OutputGroup, StreamStats as FFmpegStats } from '@spiritstream/types';
import type { AggregateStreamStats, StreamStatusType } from '@/types/stream';

// L3: re-export the backend's per-group ffmpeg stats type under the
// historical `FFmpegStats` name so call sites in this folder keep
// reading. The inline `interface FFmpegStats` that lived here
// duplicated `@spiritstream/types/StreamStats` field-for-field — that
// duplication drifted in the past (camelCase / dupFrames / size /
// speed) and silently desynced from the ts-rs source of truth. Now
// the type alias makes it impossible for the two to drift again.
export type { FFmpegStats };

export interface GroupStats {
  fps: number;
  bitrate: number;
  droppedFrames: number;
  uptime: number;
  speed: number;
}

export interface StreamState {
  isStreaming: boolean;
  activeGroups: Set<string>;
  enabledGroups: Set<string>;
  enabledTargets: Set<string>;
  stats: AggregateStreamStats;
  groupStats: Record<string, GroupStats>;
  uptime: number;
  globalStatus: StreamStatusType;
  error: string | null;
  activeStreamCount: number;

  startGroup: (group: OutputGroup, incomingUrl: string) => Promise<void>;
  stopGroup: (groupId: string) => Promise<void>;
  startAllGroups: (groups: OutputGroup[], incomingUrl: string) => Promise<void>;
  stopAllGroups: () => Promise<void>;
  toggleTargetLive: (
    targetId: string,
    enabled: boolean,
    group: OutputGroup,
    incomingUrl: string
  ) => Promise<void>;

  syncWithBackend: () => Promise<void>;

  setIsStreaming: (isStreaming: boolean) => void;
  setGroupEnabled: (groupId: string, enabled: boolean) => void;
  setTargetEnabled: (targetId: string, enabled: boolean) => void;
  updateStats: (groupId: string, ffmpegStats: FFmpegStats) => void;
  setStreamEnded: (groupId: string) => void;
  setStreamError: (groupId: string, error: string) => void;
  setUptime: (uptime: number) => void;
  incrementUptime: () => void;
  setGlobalStatus: (status: StreamStatusType) => void;
  setError: (error: string | null) => void;
  reset: () => void;
}

export const initialStats: AggregateStreamStats = {
  totalBitrate: 0,
  droppedFrames: 0,
  uptime: 0,
};
