import type { Encoders, FFmpegVersionInfo, RtmpTestResult } from '@spiritstream/types';
import { fetchTypedJson } from './_internal';

export interface EncoderPresetsResponse {
  resolutions: string[];
  fpsValues: string[];
  audioBitrates: string[];
  audioChannels: string[];
  audioSampleRates: string[];
  containerFormats: string[];
  h264Profiles: string[];
  presets: Record<string, string[]>;
  defaultPresets: Record<string, string>;
}

export interface RangeU32 {
  min: number;
  max: number;
}

export interface ClientConfigResponse {
  obsTriggerDelayMs: number;
  autoSaveDelayMs: number;
  chatPollIntervalMs: number;
  toastDurationMs: number;
  chatPopupWidth: number;
  chatPopupHeight: number;
  chatOverlayPollMs: number;
  retryBaseDelayMs: number;
  themeInitTimeoutMs: number;
  themeTokenRetryDelayMs: number;
  chatMaxChars: Record<string, number>;
  bitrateRange: RangeU32;
  keyframeRange: RangeU32;
  fpsRange: RangeU32;
  discordWebhookPrefixes: readonly string[];
  passwordMinLength: number;
}

export const system = {
  getEncoders: () => fetchTypedJson<Encoders>('GET', '/api/v1/system/encoders'),
  testFfmpeg: () => fetchTypedJson<string>('GET', '/api/v1/system/ffmpeg/test'),
  getFfmpegPath: () => fetchTypedJson<string | null>('GET', '/api/v1/system/ffmpeg/path'),
  checkFfmpegUpdate: (installedVersion?: string) =>
    fetchTypedJson<FFmpegVersionInfo>(
      'GET',
      '/api/v1/system/ffmpeg/update',
      installedVersion ? { installedVersion } : undefined,
    ),
  validateFfmpegPath: (path: string) =>
    fetchTypedJson<string>('POST', '/api/v1/system/ffmpeg/validate-path', undefined, { path }),
  testRtmpTarget: (url: string, streamKey: string) =>
    fetchTypedJson<RtmpTestResult>('POST', '/api/v1/system/rtmp/test', undefined, {
      url,
      streamKey,
    }),
  getRecentLogs: (maxLines?: number) =>
    fetchTypedJson<string[]>(
      'GET',
      '/api/v1/system/logs',
      maxLines ? { maxLines: String(maxLines) } : undefined,
    ),
  exportLogs: async (path: string, content: string) => {
    await fetchTypedJson<unknown>('POST', '/api/v1/system/logs/export', undefined, {
      path,
      content,
    });
  },
  /** Encoder preset matrix replacing `OutputGroupModal.tsx`'s hardcoded lists. */
  encoderPresets: () =>
    fetchTypedJson<EncoderPresetsResponse>('GET', '/api/v1/system/encoders/presets'),
  /** Server-tuned client constants replacing `apps/web/src/lib/constants.ts`. */
  clientConfig: () => fetchTypedJson<ClientConfigResponse>('GET', '/api/v1/system/client-config'),
  appVersion: () => fetchTypedJson<{ version: string }>('GET', '/api/v1/system/app-version'),
  recordAppUpdateFailure: async (detail: string) => {
    await fetchTypedJson<unknown>(
      'POST',
      '/api/v1/system/audit/app-update-failure',
      undefined,
      { detail },
    );
  },
};
