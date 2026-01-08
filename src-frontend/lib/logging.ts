import type { LogEntry, LogLevel } from '@/types/stream';

const logLineRegex =
  /^\[(\d{4}-\d{2}-\d{2})\]\[(\d{2}:\d{2}:\d{2})\]\[([^\]]+)\]\[(TRACE|DEBUG|INFO|WARN|ERROR)\]\s*(.*)$/;

let logIdCounter = 1;

const nextLogId = (): string => String(logIdCounter++);

const normalizeLevel = (level: string): LogLevel => {
  switch (level.toUpperCase()) {
    case 'ERROR':
      return 'error';
    case 'WARN':
      return 'warn';
    case 'DEBUG':
    case 'TRACE':
      return 'debug';
    case 'INFO':
    default:
      return 'info';
  }
};

const inferLevelFromLine = (line: string): LogLevel => {
  const upper = line.toUpperCase();
  if (upper.includes('[ERROR]') || upper.includes(' ERROR')) return 'error';
  if (upper.includes('[WARN]') || upper.includes(' WARNING') || upper.includes(' WARN')) return 'warn';
  if (upper.includes('[DEBUG]') || upper.includes('[TRACE]') || upper.includes(' DEBUG')) return 'debug';
  return 'info';
};

export const mapLogLevelFromNumber = (level: number): LogLevel => {
  // log::Level values: Error=1, Warn=2, Info=3, Debug=4, Trace=5
  switch (level) {
    case 1:
      return 'error';
    case 2:
      return 'warn';
    case 3:
      return 'info';
    case 4:
    case 5:
    default:
      return 'debug';
  }
};

export const createLogEntry = (level: LogLevel, message: string, timestamp?: Date): LogEntry => ({
  id: nextLogId(),
  timestamp: timestamp ?? new Date(),
  level,
  message,
});

export const parseLogLine = (line: string): LogEntry => {
  const match = logLineRegex.exec(line);
  if (match) {
    const [, date, time, target, levelRaw, message] = match;
    const timestamp = new Date(`${date}T${time}Z`);
    const safeTimestamp = Number.isNaN(timestamp.getTime()) ? new Date() : timestamp;
    return {
      id: nextLogId(),
      timestamp: safeTimestamp,
      level: normalizeLevel(levelRaw),
      message: `[${target}] ${message}`,
      source: target,
    };
  }

  return createLogEntry(inferLevelFromLine(line), line);
};
