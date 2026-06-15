import type { Encoders, FFmpegVersionInfo, RtmpTestResult } from '@spiritstream/types';
import {
  v1SystemEncodersProxy,
  v1SystemFfmpegTestProxy,
  v1SystemFfmpegPathProxy,
  v1SystemFfmpegUpdateProxy,
  v1SystemFfmpegValidateProxy,
  v1SystemRtmpTestProxy,
  v1SystemLogsProxy,
  v1SystemLogsExportProxy,
  v1SystemEncoderPresets,
  v1SystemClientConfig,
  v1SystemAppVersion,
  v1SystemAuditAppUpdateFailure,
} from '../generated';

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
  getEncoders: async (): Promise<Encoders> => {
    const { data } = await v1SystemEncodersProxy({ throwOnError: true });
    return data as Encoders;
  },
  testFfmpeg: async (): Promise<string> => {
    const { data } = await v1SystemFfmpegTestProxy({ throwOnError: true });
    return data.version;
  },
  getFfmpegPath: async (): Promise<string | null> => {
    const { data } = await v1SystemFfmpegPathProxy({ throwOnError: true });
    return data.path ?? null;
  },
  checkFfmpegUpdate: async (installedVersion?: string): Promise<FFmpegVersionInfo> => {
    const { data } = await v1SystemFfmpegUpdateProxy({
      query: installedVersion ? { installedVersion } : undefined,
      throwOnError: true,
    });
    return data as FFmpegVersionInfo;
  },
  validateFfmpegPath: async (path: string): Promise<string> => {
    const { data } = await v1SystemFfmpegValidateProxy({ body: { path }, throwOnError: true });
    return data.validated;
  },
  testRtmpTarget: async (url: string, streamKey: string): Promise<RtmpTestResult> => {
    const { data } = await v1SystemRtmpTestProxy({ body: { url, streamKey }, throwOnError: true });
    return data as RtmpTestResult;
  },
  getRecentLogs: async (maxLines?: number): Promise<string[]> => {
    const { data } = await v1SystemLogsProxy({
      query: maxLines ? { maxLines } : undefined,
      throwOnError: true,
    });
    return data.lines;
  },
  exportLogs: async (path: string, content: string): Promise<void> => {
    await v1SystemLogsExportProxy({ body: { path, content }, throwOnError: true });
  },
  /** Encoder preset matrix replacing `OutputGroupModal.tsx`'s hardcoded lists. */
  encoderPresets: async (): Promise<EncoderPresetsResponse> => {
    const { data } = await v1SystemEncoderPresets({ throwOnError: true });
    return data as unknown as EncoderPresetsResponse;
  },
  /** Server-tuned client constants replacing `apps/web/src/lib/constants.ts`. */
  clientConfig: async (): Promise<ClientConfigResponse> => {
    const { data } = await v1SystemClientConfig({ throwOnError: true });
    return data as unknown as ClientConfigResponse;
  },
  appVersion: async (): Promise<{ version: string }> => {
    const { data } = await v1SystemAppVersion({ throwOnError: true });
    return data;
  },
  recordAppUpdateFailure: async (detail: string): Promise<void> => {
    await v1SystemAuditAppUpdateFailure({ body: { detail }, throwOnError: true });
  },
};
