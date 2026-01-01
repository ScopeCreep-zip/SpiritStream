/**
 * Stream status types
 */
export type StreamStatusType = 'live' | 'connecting' | 'offline' | 'error';

/**
 * Statistics for a single stream target
 */
export interface TargetStats {
  viewers: number;
  bitrate: number;
  fps: number;
  status: StreamStatusType;
}

/**
 * Overall stream statistics
 */
export interface StreamStats {
  totalBitrate: number;
  droppedFrames: number;
  uptime: number; // seconds
  targetStats: Record<string, TargetStats>;
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

/**
 * Encoder types available
 */
export interface Encoders {
  video: string[];
  audio: string[];
}
