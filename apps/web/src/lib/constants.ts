// Server-tuned client constants. Backend is the source of truth: this module
// hydrates from `GET /api/v1/system/client-config` at app start, then exposes
// the values via the mutable `clientConfig` object. The hard-coded defaults
// here are fallbacks for the first paint before the hydrate call resolves
// (and for tests that don't boot the backend).
//
// This used to be ~7 hard-coded `export const`s. Consumers now read
// `clientConfig.X` so a single fetch can update all of them.

import { api } from '@/lib/client';
import { logger } from '@/lib/logger';

export const clientConfig = {
  /** Delay before triggering SpiritStream when OBS starts streaming (ms). */
  OBS_TRIGGER_DELAY_MS: 2000,
  /** Auto-save debounce delay (ms). */
  AUTO_SAVE_DELAY_MS: 500,
  /** Polling interval for refreshing chat platform status (ms). */
  CHAT_POLL_INTERVAL_MS: 5000,
  /** Default toast duration (ms). */
  TOAST_DURATION_MS: 4000,
  /** Default width for the chat popup / overlay window. */
  CHAT_POPUP_WIDTH: 420,
  /** Default height for the chat popup / overlay window. */
  CHAT_POPUP_HEIGHT: 720,
  /** Polling interval for chat overlay main-window existence check (ms). */
  CHAT_OVERLAY_POLL_MS: 500,
  /** Base delay for HTTP retry with exponential back-off (ms). */
  RETRY_BASE_DELAY_MS: 800,
  /** Hard timeout for theme initialization on app start (ms). */
  THEME_INIT_TIMEOUT_MS: 10_000,
  /** Delay between theme-token fetch retry attempts (ms). */
  THEME_TOKEN_RETRY_DELAY_MS: 500,
  /** Video bitrate bounds (kbps). Server-authoritative — frontend
   *  modal renders these for live feedback; same range is enforced
   *  on save by `StreamService::validate_config`. */
  BITRATE_MIN: 500,
  BITRATE_MAX: 50_000,
  /** Keyframe interval bounds (seconds). */
  KEYFRAME_MIN: 1,
  KEYFRAME_MAX: 10,
  /** Frames-per-second bounds. */
  FPS_MIN: 1,
  FPS_MAX: 240,
  /** Allowed Discord webhook URL prefixes. Server-authoritative —
   *  frontend renders the live-validation indicator from this list
   *  rather than hard-coding the domains. */
  DISCORD_WEBHOOK_PREFIXES: [
    'https://discord.com/api/webhooks/',
    'https://discordapp.com/api/webhooks/',
  ] as readonly string[],
  /** Minimum length for profile-encryption passwords. Server-authoritative;
   *  the frontend uses this for inline form feedback only, the server
   *  rejects shorter passwords with `CoreError::PasswordTooShort`. The
   *  default (12) matches the const exposed by `crates/core` so the
   *  fallback during pre-hydrate UX is correct. */
  PASSWORD_MIN_LENGTH: 12,
};

/**
 * Fetch the canonical values from the backend and overwrite the defaults.
 * Called once at app startup; failures are non-fatal — defaults remain.
 */
export async function hydrateClientConfig(): Promise<void> {
  try {
    const config = await api.system.clientConfig();
    clientConfig.OBS_TRIGGER_DELAY_MS = config.obsTriggerDelayMs;
    clientConfig.AUTO_SAVE_DELAY_MS = config.autoSaveDelayMs;
    clientConfig.CHAT_POLL_INTERVAL_MS = config.chatPollIntervalMs;
    clientConfig.TOAST_DURATION_MS = config.toastDurationMs;
    clientConfig.CHAT_POPUP_WIDTH = config.chatPopupWidth;
    clientConfig.CHAT_POPUP_HEIGHT = config.chatPopupHeight;
    clientConfig.CHAT_OVERLAY_POLL_MS = config.chatOverlayPollMs;
    clientConfig.RETRY_BASE_DELAY_MS = config.retryBaseDelayMs;
    clientConfig.THEME_INIT_TIMEOUT_MS = config.themeInitTimeoutMs;
    clientConfig.THEME_TOKEN_RETRY_DELAY_MS = config.themeTokenRetryDelayMs;
    clientConfig.BITRATE_MIN = config.bitrateRange.min;
    clientConfig.BITRATE_MAX = config.bitrateRange.max;
    clientConfig.KEYFRAME_MIN = config.keyframeRange.min;
    clientConfig.KEYFRAME_MAX = config.keyframeRange.max;
    clientConfig.FPS_MIN = config.fpsRange.min;
    clientConfig.FPS_MAX = config.fpsRange.max;
    clientConfig.DISCORD_WEBHOOK_PREFIXES = config.discordWebhookPrefixes;
    clientConfig.PASSWORD_MIN_LENGTH = config.passwordMinLength;
  } catch (err) {
    logger.warn('[clientConfig] hydrate failed; keeping defaults:', err);
  }
}
