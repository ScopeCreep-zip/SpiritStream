import type { OutputGroup } from '@spiritstream/types';
import type { StreamStats, StreamStatusType, TargetStats } from '@/types/stream';

/// Real-time streaming statistics from the FFmpeg backend.
///
/// Maps to `crates/core/src/models/stream_stats.rs` after serde's
/// camelCase transformation (`group_id` → `groupId`, etc.).
export interface FFmpegStats {
  groupId: string;
  frame: number;
  fps: number;
  bitrate: number;
  speed: number;
  size: number;
  time: number;
  droppedFrames: number;
  dupFrames: number;
}

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
  stats: StreamStats;
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
    incomingUrl: string,
  ) => Promise<void>;

  syncWithBackend: () => Promise<void>;
  isGroupStreamingBackend: (groupId: string) => Promise<boolean>;

  setIsStreaming: (isStreaming: boolean) => void;
  setActiveGroup: (groupId: string, active: boolean) => void;
  setGroupEnabled: (groupId: string, enabled: boolean) => void;
  toggleTarget: (targetId: string) => void;
  setTargetEnabled: (targetId: string, enabled: boolean) => void;
  updateStats: (groupId: string, ffmpegStats: FFmpegStats) => void;
  updateTargetStats: (targetId: string, stats: TargetStats) => void;
  setStreamEnded: (groupId: string) => void;
  setStreamError: (groupId: string, error: string) => void;
  setUptime: (uptime: number) => void;
  incrementUptime: () => void;
  setGlobalStatus: (status: StreamStatusType) => void;
  setError: (error: string | null) => void;
  reset: () => void;
}

export const initialStats: StreamStats = {
  totalBitrate: 0,
  droppedFrames: 0,
  uptime: 0,
  targetStats: {},
};
