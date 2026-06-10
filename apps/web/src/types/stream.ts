/**
 * Stream status types
 */
export type StreamStatusType = 'live' | 'connecting' | 'offline' | 'error';

/**
 * Aggregate UI stream statistics — rolled up across every active output
 * group for display in the dashboard header.
 *
 * L2: pre-fix this type was named `StreamStats`, shadowing the backend's
 * per-group ffmpeg stats type (`@spiritstream/types/StreamStats` — 9
 * fields including `groupId` / `frame` / `speed` / `size`). Renamed to
 * `AggregateStreamStats` so consumers can import the backend type
 * unambiguously and the two concepts stop colliding in autocomplete.
 */
export interface AggregateStreamStats {
  totalBitrate: number;
  droppedFrames: number;
  uptime: number; // seconds
}

/**
 * Stream info returned from starting a stream
 */
export interface StreamInfo {
  pid: number;
  groupId: string;
  status: StreamStatusType;
  startTime: Date;
}

/**
 * Log levels for the log console
 */
export type LogLevel = 'info' | 'warn' | 'error' | 'debug';

/**
 * Log entry structure
 */
export interface LogEntry {
  id: string;
  timestamp: Date;
  level: LogLevel;
  message: string;
  source?: string;
}

// L3: `Encoders` is the backend ts-rs export — re-export so existing
// `import { Encoders } from '@/types/stream'` call sites don't churn
// while gaining access to the `metadata` field the narrow local
// redeclaration was hiding.
export type { Encoders } from '@spiritstream/types';
