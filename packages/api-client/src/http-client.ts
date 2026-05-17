// `HttpClient` is the HTTP implementation of `ApiClient`. It delegates to the
// existing `api` object in `./api.ts`, which is the transitional
// `POST /api/v1/invoke/:command` dispatch bridge plus typed REST handlers as
// they migrate. Future transport implementations (VeilidClient)
// implement the same `ApiClient` contract against their own protocol.

import { api } from './api';
import type { ApiClient } from './interface';

export class HttpClient implements ApiClient {
  // Each resource namespace is the corresponding entry on the existing
  // typed `api` object. When an invoke command migrates to a typed
  // REST handler, the underlying body of the method in `./api.ts` changes
  // but its signature here does not.
  readonly profile = api.profile;
  readonly stream = api.stream;
  readonly system = api.system;
  readonly settings = api.settings;
  readonly theme = api.theme;
  readonly obs = api.obs;
  readonly discord = api.discord;
  readonly chat = api.chat;
  readonly oauth = api.oauth;
  readonly files = api.files;
  readonly safety = api.safety;
  readonly audit = api.audit;
  readonly security = api.security;

  constructor() {
    // No-op today. Future transports may take a config (URL, auth provider).
    // The current `api` reads `getBackendBaseUrl()` lazily per call so URL
    // changes via `updateBackendUrl()` take effect without rebuilding the client.
  }
}
