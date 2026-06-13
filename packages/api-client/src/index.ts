// Public surface of @spiritstream/api-client.
//
// Frontends instantiate one `ApiClient` via `makeApiClient(opts)` and never
// touch transport details directly. `HttpClient` is today's only impl; a
// future `VeilidClient` will satisfy the same interface.
//
// Lower-level utilities (`events`, `dialogs`, auth helpers, URL config)
// remain exported for code that doesn't fit the resource-API shape
// (WebSocket subscription, file pickers, session login). These get folded
// into the `ApiClient` surface as their typed REST equivalents land.

import { HttpClient } from './http-client';
import type { ApiClient, ApiClientOptions } from './interface';

export type {
  ApiClient,
  ApiClientOptions,
  ProfileApi,
  StreamApi,
  SystemApi,
  SettingsApi,
  ThemeApi,
  ObsApi,
  DiscordApi,
  ChatApi,
  OAuthApi,
  FilesApi,
  StreamValidationFailure,
  Transport,
  HttpTransportOptions,
} from './interface';
export type {
  FileEntry,
  FileBrowseResponse,
  FileHomeResponse,
  EncoderPresetsResponse,
  ClientConfigResponse,
  AuditChainStatus,
  AuditLogResponse,
} from './api';

/// Build an `ApiClient` for the given transport. The current implementation
/// only supports `'http'`; `'veilid'` arrives later.
export function makeApiClient(opts: ApiClientOptions): ApiClient {
  switch (opts.transport) {
    case 'http':
      return new HttpClient();
  }
}

// Direct exports for sub-surfaces not yet covered by `ApiClient` and for
// the codegen-generated raw HTTP client.
export { events, initConnection, disconnectSocket } from './events';
export { dialogs } from './dialogs';
export type { OpenFileOptions, SaveFileOptions, OpenTextResult, DialogFilter } from './dialogTypes';
export type { OAuthProviderSummary, OAuthFlowStarted } from './api/oauth';
export {
  backendMode,
  backendUrlStorageKey,
  getBackendBaseUrl,
  getBackendWsUrl,
  setBackendBaseUrl,
  updateBackendUrl,
  clearBackendUrl,
  checkAuth,
  login,
  isTauri,
  getAuthHeaders,
  safeFetch,
} from './config';
export type { AuthStatus, BackendMode, ServerReadyError, ServerReadyStatus } from './config';
