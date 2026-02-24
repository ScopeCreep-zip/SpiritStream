/* eslint-disable no-console */
type LogLevel = 'debug' | 'info' | 'warn' | 'error';

const LOG_LEVELS: Record<LogLevel, number> = { debug: 0, info: 1, warn: 2, error: 3 };
const currentLevel: LogLevel = import.meta.env.DEV ? 'debug' : 'warn';

export const logger = {
  debug: (...args: unknown[]) => {
    if (LOG_LEVELS.debug >= LOG_LEVELS[currentLevel]) console.debug('[SS]', ...args);
  },
  info: (...args: unknown[]) => {
    if (LOG_LEVELS.info >= LOG_LEVELS[currentLevel]) console.info('[SS]', ...args);
  },
  warn: (...args: unknown[]) => {
    if (LOG_LEVELS.warn >= LOG_LEVELS[currentLevel]) console.warn('[SS]', ...args);
  },
  error: (...args: unknown[]) => {
    if (LOG_LEVELS.error >= LOG_LEVELS[currentLevel]) console.error('[SS]', ...args);
  },
};
