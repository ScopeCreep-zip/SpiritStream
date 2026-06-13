// The transport-polymorphic `ApiClient` interface. `HttpClient` is the only
// implementation today; a future Veilid-backed implementation will satisfy
// the same contract. Frontends program against this interface only; they
// never see HTTP details (URLs, fetch options, cookie credentials).
//
// The shape mirrors the SpiritStream domain resources: profiles, streams,
// chat, oauth, system, settings, theme, obs, discord. Each resource is a
// nested namespace of typed async methods that throw on failure (caller
// catches `Error`; the underlying body is the JSON-serialized `CoreError`
// shape from `@spiritstream/types`).
//
// Method signatures are transport-stable: the `HttpClient` implementation
// delegates to the typed REST endpoints, and a future transport swaps the
// delegation without touching callers.

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
  Settings,
  ChatConfig,
  ChatPlatform,
  ChatPlatformStatus,
  ChatSendResult,
  ChatMessage,
  ChatLogStatus,
  OAuthAccountStatus,
  ValidationIssue,
  FileBrowseResponse,
  FileHomeResponse,
} from '@spiritstream/types';
import type { EncoderPresetsResponse, ClientConfigResponse } from './api';

export interface ProfileApi {
  getAll(): Promise<string[]>;
  getSummaries(): Promise<ProfileSummary[]>;
  load(name: string, password?: string, setActive?: boolean): Promise<Profile>;
  /** Load + set-active + emit `profile_activated` consolidated event. */
  activate(name: string, password?: string): Promise<Profile>;
  unlock(name: string, password: string): Promise<{ name: string; unlocked: boolean }>;
  /** Atomic encryption-removal (load with password + save unencrypted in one call). */
  decrypt(name: string, password: string): Promise<{ name: string; decrypted: boolean }>;
  lock(name: string): Promise<{ name: string; locked: boolean }>;
  lockedList(): Promise<{ unlocked: string[] }>;
  /** Returns the canonical persisted profile (server-computed fields refreshed) — adopt it. */
  save(profile: Profile, password?: string): Promise<Profile>;
  delete(name: string): Promise<void>;
  isEncrypted(name: string): Promise<boolean>;
  validateInput(profileId: string, input: RtmpInput): Promise<void>;
  setProfileOrder(orderedNames: string[]): Promise<void>;
  getOrderIndexMap(): Promise<Record<string, number>>;
  ensureOrderIndexes(): Promise<Record<string, number>>;
}

export interface StreamApi {
  start(group: OutputGroup, incomingUrl: string): Promise<number>;
  startAll(
    groups: OutputGroup[],
    incomingUrl: string
  ): Promise<{ pids: number[]; startedGroupIds: string[] }>;
  stop(groupId: string): Promise<void>;
  stopAll(): Promise<void>;
  getActiveCount(): Promise<number>;
  getActiveGroupIds(): Promise<string[]>;
  toggleTarget(
    targetId: string,
    enabled: boolean,
    group: OutputGroup,
    incomingUrl: string
  ): Promise<number>;
  isTargetDisabled(targetId: string): Promise<boolean>;
  retry(groupId: string): Promise<{ pid: number; nextDelaySecs: number | null }>;
  /**
   * Server-side encoding-config validation. Throws when invalid; caller can
   * read `(err as Error & { kind?: string; details?: { reasons?: ValidationIssue[] } })`
   * to surface field-level messages.
   */
  validate(profile: Profile): Promise<{ valid: boolean }>;
}

/** Convenience alias for the wire shape of `CoreError::InvalidStreamConfig`. */
export type StreamValidationFailure = Error & {
  kind?: string;
  details?: { reasons?: ValidationIssue[] };
};

export interface SystemApi {
  getEncoders(): Promise<Encoders>;
  testFfmpeg(): Promise<string>;
  getFfmpegPath(): Promise<string | null>;
  checkFfmpegUpdate(installedVersion?: string): Promise<FFmpegVersionInfo>;
  validateFfmpegPath(path: string): Promise<string>;
  testRtmpTarget(url: string, streamKey: string): Promise<RtmpTestResult>;
  getRecentLogs(maxLines?: number): Promise<string[]>;
  exportLogs(path: string, content: string): Promise<void>;
  encoderPresets(): Promise<EncoderPresetsResponse>;
  clientConfig(): Promise<ClientConfigResponse>;
  /**
   * Running app version (semver). Sourced from Cargo metadata at
   * compile time. Used by the About section so bug reports show the
   * actual running version rather than a hardcoded constant.
   */
  appVersion(): Promise<{ version: string }>;
  /**
   * Record a self-updater signature / download failure into the
   * HMAC-chained audit log. Called by the frontend when the Tauri
   * updater rejects a `.sig` or otherwise fails — gives operators a
   * single grep target (`app_update_signature_failed`) for tampered-
   * update attempts.
   */
  recordAppUpdateFailure(detail: string): Promise<void>;
}

export interface SettingsApi {
  get(): Promise<Settings>;
  save(settings: Settings): Promise<void>;
  getProfilesPath(): Promise<string>;
  exportData(exportPath: string): Promise<void>;
  clearData(): Promise<void>;
  rotateMachineKey(unlockedPasswords?: Record<string, string>): Promise<RotationReport>;
}

export interface ThemeApi {
  list(): Promise<ThemeSummary[]>;
  getTokens(themeId: string): Promise<Record<string, string>>;
  install(themePath: string): Promise<ThemeSummary>;
  refresh(): Promise<ThemeSummary[]>;
}

export interface ObsApi {
  getState(): Promise<ObsState>;
  /** Never carries the password value — only whether one is set. */
  getConfig(): Promise<Omit<ObsConfig, 'password'> & { hasPassword: boolean }>;
  setConfig(config: {
    host: string;
    port: number;
    password?: string;
    useAuth: boolean;
    direction: ObsIntegrationDirection;
    autoConnect: boolean;
  }): Promise<void>;
  connect(): Promise<void>;
  disconnect(): Promise<void>;
  startStream(): Promise<void>;
  stopStream(): Promise<void>;
  isConnected(): Promise<boolean>;
}

export interface DiscordApi {
  testWebhook(
    url: string
  ): Promise<{ success: boolean; message: string; skippedCooldown: boolean }>;
  sendNotification(): Promise<{ success: boolean; message: string; skippedCooldown: boolean }>;
  resetCooldown(): Promise<void>;
}

export interface ChatApi {
  connect(config: ChatConfig): Promise<void>;
  /** Confirm-token-gated Facebook connect — see api/chat.ts for the
   *  identity-warning + token flow. Identical payload shape to
   *  `connect`, separate method so the gate is unmissable. */
  connectFacebook(config: ChatConfig): Promise<unknown>;
  /**
   * Send `message` to chat. Without `targetPlatforms` the backend
   * dispatches to every platform whose `*_send_enabled` flag is on —
   * the broadcast behaviour gated by `chatSettings.sendAllEnabled`.
   * With `targetPlatforms` set, dispatch only to that list — core
   * re-checks the `*_send_enabled` flags and the connector's
   * `can_send()` gate either way, so an explicit list can only narrow
   * the broadcast set. The composer passes the array when the user has
   * flipped sendAllEnabled off and picked a single platform.
   */
  sendMessage(message: string, targetPlatforms?: ChatPlatform[]): Promise<ChatSendResult[]>;
  disconnect(platform: ChatPlatform): Promise<void>;
  retryConnection(platform: ChatPlatform): Promise<void>;
  disconnectAll(): Promise<void>;
  getStatus(): Promise<ChatPlatformStatus[]>;
  getLogStatus(): Promise<ChatLogStatus>;
  exportLog(path: string): Promise<void>;
  searchSession(query: string, limit?: number): Promise<ChatMessage[]>;
  getPlatformStatus(platform: ChatPlatform): Promise<ChatPlatformStatus | null>;
  isConnected(): Promise<boolean>;
}

export interface FilesApi {
  browse(path?: string): Promise<FileBrowseResponse>;
  home(): Promise<FileHomeResponse>;
  open(path: string): Promise<void>;
}

export interface OAuthApi {
  isConfigured(provider: string): Promise<boolean>;
  /** Starts the provider's flow. The backend chooses the grant: a
   *  `redirect` response carries `authUrl` (+ `browserOpened`), a
   *  `device` response carries `userCode`/`verificationUri` to render. */
  startFlow(provider: string): Promise<import('./api/oauth').OAuthFlowStarted>;
  completeFlow(
    provider: string,
    code: string,
    state: string
  ): Promise<{
    provider: string;
    userId: string;
    username: string;
    displayName: string;
  }>;
  getAccount(provider: string): Promise<OAuthAccountStatus>;
  disconnect(provider: string): Promise<void>;
  forget(provider: string): Promise<void>;
  refreshToken(
    provider: string,
    refreshToken: string
  ): Promise<{
    accessToken: string;
    refreshToken?: string;
    expiresIn?: number;
  }>;
  /** Truthful per-provider setup summaries (placeholders report unconfigured). */
  getConfig(): Promise<import('./api/oauth').OAuthProviderSummary[]>;
  /** Store one provider's client credentials (in-app setup form); persists. */
  setProviderCredentials(
    provider: string,
    credentials: { clientId?: string; clientSecret?: string }
  ): Promise<import('./api/oauth').OAuthProviderSummary[]>;
  setConfig(config: {
    twitchClientId?: string;
    twitchClientSecret?: string;
    youtubeClientId?: string;
    youtubeClientSecret?: string;
    kickClientId?: string;
    kickClientSecret?: string;
    facebookClientId?: string;
    facebookClientSecret?: string;
    trovoClientId?: string;
    trovoClientSecret?: string;
  }): Promise<void>;
}

/// The transport-polymorphic SpiritStream client contract.
///
/// **Implementations**:
/// - `HttpClient` (this package, `./http-client.ts`) — REST over `/api/v1/*`.
/// - Future: `VeilidClient` — DHT-routed RPC. Same interface.
///
/// **Stability**: methods may add fields to inputs/outputs (additive only).
/// Removing or renaming a method is a breaking change for every frontend.
export interface ApiClient {
  profile: ProfileApi;
  stream: StreamApi;
  system: SystemApi;
  settings: SettingsApi;
  theme: ThemeApi;
  obs: ObsApi;
  discord: DiscordApi;
  chat: ChatApi;
  oauth: OAuthApi;
  files: FilesApi;
  // Safety/audit endpoints.
  safety: {
    panic(): Promise<{ streamsStopped: number; elapsedMs: number }>;
  };
  audit: {
    log(opts?: {
      skip?: number;
      limit?: number;
      kind?: string;
    }): Promise<import('./api/audit').AuditLogResponse>;
  };
  // Destructive-op confirmation tokens.
  security: {
    /**
     * End the current session and expire its cookie. Other devices'
     * sessions stay valid — use `revokeAllSessions` for those.
     */
    logout(): Promise<Record<string, never>>;
    /**
     * Low-level: request a one-shot confirm token for `intent`.
     * Most callers use the wrapped destructive methods
     * (`settings.clearData`, `settings.rotateMachineKey`,
     * `security.revokeAllSessions`) which do the dance internally.
     */
    requestConfirmToken(intent: string): Promise<{ token: string; expiresInSeconds: number }>;
    /**
     * Revoke every active server-side session. The caller's own
     * session is invalidated by this call.
     */
    revokeAllSessions(): Promise<{ revoked: number }>;
  };
}

export type Transport = 'http';

export interface HttpTransportOptions {
  transport: 'http';
  /** Override the backend URL. Resolution otherwise: runtime-discovered
   *  (Tauri `backend_url` command, OS-negotiated port) → localStorage →
   *  VITE_BACKEND_URL → inferred (PROD browser: page origin; dev:
   *  `http://127.0.0.1:8008`). */
  url?: string;
}

export type ApiClientOptions = HttpTransportOptions;
