import type {
  Profile,
  ProfileSummary,
  OutputGroup,
  RtmpInput,
  Encoders,
  ThemeSummary,
  FFmpegVersionInfo,
  RotationReport,
  RtmpTestResult,
  ObsConfig,
  ObsState,
  ObsIntegrationDirection,
  Settings as AppSettings,
  ChatConfig,
  ChatPlatform,
  ChatPlatformStatus,
  ChatSendResult,
  ChatMessage,
  FileBrowseResponse,
  FileHomeResponse,
} from "@spiritstream/types";
import type { ChatLogStatus, OAuthAccountStatus, OAuthFlowResult } from "@spiritstream/types";
import { getAuthHeaders, getBackendBaseUrl, safeFetch } from './config';

export type { FileBrowseResponse, FileHomeResponse, FileEntry } from "@spiritstream/types";

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

/**
 * Issue a typed-REST request and return the response body.
 *
 * Success: raw `T` from the handler's `Json<T>` response.
 * Error: `{ kind, details? }` per `ApiError(CoreError)::IntoResponse` in
 * `crates/transport-http/src/error.rs`. The thrown `Error` carries
 * `(err as Error & { kind, status, details? })` so callers can branch on
 * a specific validation failure without parsing the message string.
 */
async function fetchTypedJson<T>(
  method: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE',
  path: string,
  query?: Record<string, string>,
  body?: unknown,
  /**
   * Extra request headers. Used for one-shot confirmation tokens
   * attached as `X-Confirm-Token` on destructive endpoints.
   * `getAuthHeaders()` still drives the session cookie / bearer; this
   * merges on top.
   */
  extraHeaders?: Record<string, string>,
): Promise<T> {
  const baseUrl = getBackendBaseUrl();
  const qs = query ? `?${new URLSearchParams(query).toString()}` : '';
  const headers: Record<string, string> = {
    ...getAuthHeaders(),
    ...(extraHeaders ?? {}),
  };
  if (body !== undefined) headers['Content-Type'] = 'application/json';

  const response = await safeFetch(`${baseUrl}${path}${qs}`, {
    method,
    headers,
    credentials: 'include',
    body: body === undefined ? undefined : JSON.stringify(body),
  });

  const text = await response.text();
  let parsed: unknown;
  if (text) {
    try { parsed = JSON.parse(text); }
    catch { throw new Error('Invalid response from server'); }
  }

  if (!response.ok) {
    // Typed REST error: { kind, details? }
    if (parsed && typeof parsed === 'object' && 'kind' in parsed) {
      const body = parsed as { kind: string; message?: string; details?: unknown };
      const err = new Error(body.message ?? body.kind) as Error & { kind: string; status: number; details?: unknown };
      err.kind = body.kind;
      err.status = response.status;
      err.details = body.details;
      throw err;
    }
    // Bodyless HTTP error (405 Method Not Allowed, 502/504 from proxies,
    // etc). Tag the thrown Error with `kind` + `status` so callers can
    // branch instead of string-matching the `statusText` fallback.
    const kind = response.status === 405
      ? 'method_not_allowed'
      : response.status === 401
        ? 'unauthorized'
        : response.status === 403
          ? 'forbidden'
          : response.status === 404
            ? 'not_found'
            : response.status === 429
              ? 'rate_limited'
              : response.status >= 500
                ? 'server_error'
                : 'http_error';
    const err = new Error(
      `${response.status} ${response.statusText || 'HTTP error'} for ${method} ${path}`,
    ) as Error & { kind: string; status: number };
    err.kind = kind;
    err.status = response.status;
    throw err;
  }

  // Typed REST success: handler's `Json<T>` value. Caller handles
  // `undefined` when `T = void` (bodyless 200/204 responses).
  return parsed as T;
}

/**
 * Wrap a destructive backend call with the confirm-token dance:
 *   1. Request a one-shot token for the given `intent` from
 *      `POST /api/v1/security/confirm-token`. Tokens are scoped to a
 *      single intent string (`clear_data`, `rotate_machine_key`,
 *      `revoke_all_sessions`) and expire after a short TTL.
 *   2. Invoke `call(headers)` with the token attached as
 *      `X-Confirm-Token`. The backend's `require_confirm_token`
 *      middleware (`crates/transport-http/src/lib.rs:483`) consumes
 *      the token before the handler runs.
 *
 * Two-step pattern keeps the api-client side ergonomic: callers
 * just `withConfirmToken('intent', headers => fetchTypedJson(..., headers))`
 * without juggling the token issuance themselves.
 */
async function withConfirmToken<T>(
  intent: string,
  call: (headers: Record<string, string>) => Promise<T>,
): Promise<T> {
  const { token } = await fetchTypedJson<{ token: string; expiresInSeconds: number }>(
    'POST',
    '/api/v1/security/confirm-token',
    undefined,
    { intent },
  );
  return call({ 'X-Confirm-Token': token });
}

/**
 * HTTP API wrapper that mirrors the SpiritStream typed REST surface.
 * All requests include credentials (cookies) for authentication.
 */
export const api = {
  profile: {
    getAll: async () => {
      const { names } = await fetchTypedJson<{ names: string[] }>('GET', '/api/v1/profiles');
      return names;
    },
    getSummaries: () => fetchTypedJson<ProfileSummary[]>('GET', '/api/v1/profiles/summaries'),
    load: (name: string, password?: string, _setActive: boolean = true) =>
      fetchTypedJson<Profile>(
        'GET',
        `/api/v1/profiles/${encodeURIComponent(name)}`,
        password ? { password } : undefined,
      ),
    /**
     * Load + set-active in one round-trip. Server emits `profile_activated`
     * with consolidated state — UI stores listen for the event instead of
     * running the old `applyProfileSettings` cascade themselves.
     */
    activate: (name: string, password?: string) =>
      fetchTypedJson<Profile>('POST', `/api/v1/profiles/${encodeURIComponent(name)}/activate`, undefined, { password }),
    /** Unlock an encrypted profile in the server-side session unlock set. */
    unlock: (name: string, password: string) =>
      fetchTypedJson<{ name: string; unlocked: boolean }>('POST', `/api/v1/profiles/${encodeURIComponent(name)}/unlock`, undefined, { password }),
    /**
     * Atomic encryption removal: load with password + re-save without it
     * in one server call. Replaces the legacy two-round-trip
     * `loadProfile(password) → saveProfile(no password)` flow.
     */
    decrypt: (name: string, password: string) =>
      fetchTypedJson<{ name: string; decrypted: boolean }>('POST', `/api/v1/profiles/${encodeURIComponent(name)}/decrypt`, undefined, { password }),
    /** Remove a profile from the server-side session unlock set. */
    lock: (name: string) =>
      fetchTypedJson<{ name: string; locked: boolean }>('POST', `/api/v1/profiles/${encodeURIComponent(name)}/lock`),
    /** List every encrypted profile currently unlocked in the session. */
    lockedList: () =>
      fetchTypedJson<{ unlocked: string[] }>('GET', '/api/v1/profiles/locked'),
    save: async (profile: Profile, password?: string) => {
      await fetchTypedJson<{ saved: boolean }>(
        'PUT',
        `/api/v1/profiles/${encodeURIComponent(profile.name)}`,
        undefined,
        { profile, password },
      );
    },
    delete: async (name: string) => {
      await fetchTypedJson<{ deleted: boolean }>(
        'DELETE',
        `/api/v1/profiles/${encodeURIComponent(name)}`,
      );
    },
    isEncrypted: async (name: string) => {
      const { encrypted } = await fetchTypedJson<{ encrypted: boolean }>(
        'GET',
        `/api/v1/profiles/${encodeURIComponent(name)}/encrypted`,
      );
      return encrypted;
    },
    validateInput: async (profileId: string, input: RtmpInput) => {
      await fetchTypedJson<unknown>('POST', '/api/v1/profiles/validate-input', undefined, {
        profileId,
        input,
      });
    },
    setProfileOrder: async (orderedNames: string[]) => {
      await fetchTypedJson<unknown>('PATCH', '/api/v1/profiles/order', undefined, { orderedNames });
    },
    getOrderIndexMap: () => fetchTypedJson<Record<string, number>>('GET', '/api/v1/profiles/order'),
    ensureOrderIndexes: () =>
      fetchTypedJson<Record<string, number>>('POST', '/api/v1/profiles/order/ensure'),
  },
  stream: {
    /** Start streaming for a single output group. Returns the FFmpeg process PID */
    start: async (group: OutputGroup, incomingUrl: string) => {
      const { pid } = await fetchTypedJson<{ pid: number }>(
        'POST',
        `/api/v1/streams/groups/${encodeURIComponent(group.id)}`,
        undefined,
        { group, incomingUrl },
      );
      return pid;
    },
    /** Start all output groups. Returns array of FFmpeg process PIDs */
    startAll: async (groups: OutputGroup[], incomingUrl: string) => {
      const { pids } = await fetchTypedJson<{ pids: number[] }>(
        'POST',
        '/api/v1/streams',
        undefined,
        { groups, incomingUrl },
      );
      return pids;
    },
    /** Stop streaming for a specific output group */
    stop: async (groupId: string) => {
      await fetchTypedJson<{ stopped: boolean }>(
        'DELETE',
        `/api/v1/streams/groups/${encodeURIComponent(groupId)}`,
      );
    },
    /** Stop all active streams */
    stopAll: async () => {
      await fetchTypedJson<{ stopped: boolean }>('DELETE', '/api/v1/streams');
    },
    getActiveCount: async () => {
      const status = await fetchTypedJson<{ activeCount: number; activeGroupIds: string[] }>(
        'GET',
        '/api/v1/streams',
      );
      return status.activeCount;
    },
    isGroupStreaming: async (groupId: string) => {
      const status = await fetchTypedJson<{ activeGroupIds: string[] }>(
        'GET',
        '/api/v1/streams',
      );
      return status.activeGroupIds.includes(groupId);
    },
    getActiveGroupIds: async () => {
      const status = await fetchTypedJson<{ activeGroupIds: string[] }>(
        'GET',
        '/api/v1/streams',
      );
      return status.activeGroupIds;
    },
    toggleTarget: async (targetId: string, enabled: boolean, group: OutputGroup, incomingUrl: string) => {
      const { pid } = await fetchTypedJson<{ pid: number }>(
        'PATCH',
        `/api/v1/streams/targets/${encodeURIComponent(targetId)}`,
        undefined,
        { enabled, group, incomingUrl },
      );
      return pid;
    },
    isTargetDisabled: (targetId: string) =>
      fetchTypedJson<boolean>(
        'GET',
        `/api/v1/streams/targets/${encodeURIComponent(targetId)}/disabled`,
      ),
    /** Retry a failed stream. Returns PID and next delay if another retry is needed */
    retry: (groupId: string) =>
      fetchTypedJson<{ pid: number; nextDelaySecs: number | null }>(
        'POST',
        `/api/v1/streams/groups/${encodeURIComponent(groupId)}/retry`,
      ),
    /**
     * Validate an entire profile's encoding config server-side. Throws on
     * failure with `err.kind === 'invalid_stream_config'` and `err.details.reasons`
     * carrying every `ValidationIssue`. Used for decorative live feedback in
     * modals; the same check runs inside `stream.start`.
     */
    validate: (profile: Profile) =>
      fetchTypedJson<{ valid: boolean }>('POST', '/api/v1/streams/validate', undefined, { profile }),
  },
  system: {
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
      fetchTypedJson<RtmpTestResult>(
        'POST',
        '/api/v1/system/rtmp/test',
        undefined,
        { url, streamKey },
      ),
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
    clientConfig: () =>
      fetchTypedJson<ClientConfigResponse>('GET', '/api/v1/system/client-config'),
    appVersion: () =>
      fetchTypedJson<{ version: string }>('GET', '/api/v1/system/app-version'),
    recordAppUpdateFailure: async (detail: string) => {
      await fetchTypedJson<unknown>(
        'POST',
        '/api/v1/system/audit/app-update-failure',
        undefined,
        { detail },
      );
    },
  },
  settings: {
    get: () => fetchTypedJson<AppSettings>('GET', '/api/v1/settings'),
    save: async (settings: AppSettings) => {
      await fetchTypedJson<{ saved: boolean }>('PUT', '/api/v1/settings', undefined, { settings });
    },
    getProfilesPath: async () => {
      const { path } = await fetchTypedJson<{ path: string }>('GET', '/api/v1/settings/profiles-path');
      return path;
    },
    exportData: async (exportPath: string) => {
      await fetchTypedJson<{ exported: boolean }>(
        'POST',
        '/api/v1/settings/export',
        undefined,
        { exportPath },
      );
    },
    clearData: async () => {
      // Destructive op gated by one-shot confirm token.
      // The backend (`DELETE /api/v1/settings/data` at
      // crates/transport-http/src/v1.rs::v1_settings_clear_data) calls
      // `require_confirm_token(state, headers, "clear_data")` which
      // rejects requests missing `X-Confirm-Token`. We acquire the
      // token + attach in one helper call.
      await withConfirmToken<{ cleared: boolean }>('clear_data', (headers) =>
        fetchTypedJson('DELETE', '/api/v1/settings/data', undefined, undefined, headers),
      );
    },
    rotateMachineKey: (unlockedPasswords: Record<string, string> = {}) =>
      withConfirmToken<RotationReport>('rotate_machine_key', (headers) =>
        fetchTypedJson(
          'POST',
          '/api/v1/security/machine-key/rotate',
          undefined,
          { unlockedPasswords },
          headers,
        ),
      ),
  },
  theme: {
    list: () => fetchTypedJson<ThemeSummary[]>('GET', '/api/v1/themes'),
    getTokens: (themeId: string) =>
      fetchTypedJson<Record<string, string>>(
        'GET',
        `/api/v1/themes/${encodeURIComponent(themeId)}/tokens`,
      ),
    install: (themePath: string) =>
      fetchTypedJson<ThemeSummary>('POST', '/api/v1/themes', undefined, { themePath }),
    refresh: () => fetchTypedJson<ThemeSummary[]>('POST', '/api/v1/themes/refresh'),
  },
  obs: {
    getState: () => fetchTypedJson<ObsState>('GET', '/api/v1/obs/state'),
    getConfig: () => fetchTypedJson<ObsConfig>('GET', '/api/v1/obs/config'),
    setConfig: async (config: {
      host: string;
      port: number;
      password?: string;
      useAuth: boolean;
      direction: ObsIntegrationDirection;
      autoConnect: boolean;
    }) => {
      await fetchTypedJson<unknown>('PUT', '/api/v1/obs/config', undefined, config);
    },
    connect: async () => {
      await fetchTypedJson<unknown>('POST', '/api/v1/obs/connection');
    },
    disconnect: async () => {
      await fetchTypedJson<unknown>('DELETE', '/api/v1/obs/connection');
    },
    startStream: async () => {
      await fetchTypedJson<unknown>('POST', '/api/v1/obs/stream');
    },
    stopStream: async () => {
      await fetchTypedJson<unknown>('DELETE', '/api/v1/obs/stream');
    },
    isConnected: () => fetchTypedJson<boolean>('GET', '/api/v1/obs/connection'),
  },
  discord: {
    testWebhook: (url: string) =>
      fetchTypedJson<{ success: boolean; message: string; skippedCooldown: boolean }>(
        'POST',
        '/api/v1/discord/webhook/test',
        undefined,
        { url },
      ),
    sendNotification: () =>
      fetchTypedJson<{ success: boolean; message: string; skippedCooldown: boolean }>(
        'POST',
        '/api/v1/discord/webhook/send',
      ),
    resetCooldown: async () => {
      await fetchTypedJson<unknown>('DELETE', '/api/v1/discord/webhook/cooldown');
    },
  },
  chat: {
    connect: async (config: ChatConfig) => {
      await fetchTypedJson<unknown>('POST', '/api/v1/chat/connections', undefined, { config });
    },
    sendMessage: (message: string) =>
      fetchTypedJson<ChatSendResult[]>('POST', '/api/v1/chat/messages', undefined, { message }),
    disconnect: async (platform: ChatPlatform) => {
      await fetchTypedJson<unknown>(
        'DELETE',
        `/api/v1/chat/connections/${encodeURIComponent(String(platform))}`,
      );
    },
    retryConnection: async (platform: ChatPlatform) => {
      await fetchTypedJson<unknown>(
        'POST',
        `/api/v1/chat/connections/${encodeURIComponent(String(platform))}/retry`,
      );
    },
    disconnectAll: async () => {
      await fetchTypedJson<unknown>('DELETE', '/api/v1/chat/connections');
    },
    getStatus: () =>
      fetchTypedJson<ChatPlatformStatus[]>('GET', '/api/v1/chat/connections'),
    getLogStatus: () => fetchTypedJson<ChatLogStatus>('GET', '/api/v1/chat/log'),
    exportLog: async (path: string) => {
      await fetchTypedJson<unknown>('POST', '/api/v1/chat/log/export', undefined, { path });
    },
    searchSession: (query: string, limit?: number) =>
      fetchTypedJson<ChatMessage[]>('POST', '/api/v1/chat/log/search', undefined, { query, limit }),
    getPlatformStatus: (platform: ChatPlatform) =>
      fetchTypedJson<ChatPlatformStatus | null>(
        'GET',
        `/api/v1/chat/connections/${encodeURIComponent(String(platform))}`,
      ),
    isConnected: () => fetchTypedJson<boolean>('GET', '/api/v1/chat/connected'),
  },
  oauth: {
    isConfigured: (provider: string) =>
      fetchTypedJson<boolean>('GET', `/api/v1/oauth/${encodeURIComponent(provider)}/configured`),
    startFlow: (provider: string) =>
      fetchTypedJson<OAuthFlowResult>(
        'POST',
        `/api/v1/oauth/${encodeURIComponent(provider)}/flow`,
      ),
    completeFlow: (provider: string, code: string, state: string) =>
      fetchTypedJson<{
        provider: string;
        userId: string;
        username: string;
        displayName: string;
      }>('POST', `/api/v1/oauth/${encodeURIComponent(provider)}/complete`, undefined, {
        code,
        state,
      }),
    getAccount: (provider: string) =>
      fetchTypedJson<OAuthAccountStatus>('GET', `/api/v1/oauth/${encodeURIComponent(provider)}/account`),
    disconnect: async (provider: string) => {
      await fetchTypedJson<unknown>(
        'DELETE',
        `/api/v1/oauth/${encodeURIComponent(provider)}/account`,
      );
    },
    forget: async (provider: string) => {
      await fetchTypedJson<unknown>(
        'POST',
        `/api/v1/oauth/${encodeURIComponent(provider)}/forget`,
      );
    },
    refreshToken: (provider: string, refreshToken: string) =>
      fetchTypedJson<{
        accessToken: string;
        refreshToken?: string;
        expiresIn?: number;
      }>(
        'POST',
        `/api/v1/oauth/${encodeURIComponent(provider)}/refresh`,
        undefined,
        { refreshToken },
      ),
    getConfig: () =>
      fetchTypedJson<{ twitchConfigured: boolean; youtubeConfigured: boolean }>(
        'GET',
        '/api/v1/oauth/config',
      ),
    setConfig: async (config: {
      twitchClientId?: string;
      twitchClientSecret?: string;
      youtubeClientId?: string;
      youtubeClientSecret?: string;
    }) => {
      await fetchTypedJson<unknown>('PUT', '/api/v1/oauth/config', undefined, config);
    },
  },
  files: {
    /** Browse a directory. Empty path returns server-side default (typically home). */
    browse: (path?: string) =>
      fetchTypedJson<FileBrowseResponse>('GET', '/api/v1/files/browse', path ? { path } : undefined),
    /** Resolve the home directory for the current user. */
    home: () => fetchTypedJson<FileHomeResponse>('GET', '/api/v1/files/home'),
    /** Open a path in the OS file manager / default application. */
    open: (path: string) => fetchTypedJson<void>('POST', '/api/v1/files/open', undefined, { path }),
  },
  // Safety + audit log endpoints.
  safety: {
    /**
     * Trigger panic disconnect: stops every active stream, disconnects
     * chat + OBS, wipes in-memory secret caches, records an audit
     * entry. Returns the action summary.
     */
    panic: () =>
      fetchTypedJson<{ streamsStopped: number; elapsedMs: number }>(
        'POST',
        '/api/v1/safety/panic',
      ),
  },
  audit: {
    /**
     * Read the audit log. Pagination via `skip` + `limit`; optional
     * `kind` filter (e.g. `"panic_triggered"`, `"oauth_refresh"`).
     */
    log: (opts?: { skip?: number; limit?: number; kind?: string }) => {
      const params: Record<string, string> = {};
      if (opts?.skip !== undefined) params.skip = String(opts.skip);
      if (opts?.limit !== undefined) params.limit = String(opts.limit);
      if (opts?.kind !== undefined) params.kind = opts.kind;
      return fetchTypedJson<{ total: number; entries: unknown[] }>(
        'GET',
        '/api/v1/audit/log',
        Object.keys(params).length > 0 ? params : undefined,
      );
    },
  },
  // Confirmation tokens for destructive ops.
  security: {
    /**
     * Request a one-shot confirmation token for the given destructive
     * intent. The token expires after `expiresInSeconds` and is
     * consumed by the destructive endpoint's `X-Confirm-Token` header.
     *
     * Most callers don't need this directly — use the wrapped
     * destructive methods (`settings.clearData`,
     * `settings.rotateMachineKey`, `security.revokeAllSessions`)
     * which do the token dance internally via `withConfirmToken`.
     * Exposed for the rare case a custom intent needs custom handling.
     */
    requestConfirmToken: (intent: string) =>
      fetchTypedJson<{ token: string; expiresInSeconds: number }>(
        'POST',
        '/api/v1/security/confirm-token',
        undefined,
        { intent },
      ),
    /**
     * Revoke every active session server-side, effectively logging out
     * every device that holds a session cookie / bearer token. The
     * caller's own session is invalidated by this call; the UI should
     * route the user to the login screen immediately after.
     *
     * Wires the same confirm-token flow as `clearData` /
     * `rotateMachineKey`. Backend route:
     * `POST /api/v1/security/sessions/revoke-all` at
     * `crates/transport-http/src/lib.rs:1748`.
     */
    revokeAllSessions: () =>
      withConfirmToken<{ revoked: number }>('revoke_all_sessions', (headers) =>
        fetchTypedJson(
          'POST',
          '/api/v1/security/sessions/revoke-all',
          undefined,
          undefined,
          headers,
        ),
      ),
  },
};
