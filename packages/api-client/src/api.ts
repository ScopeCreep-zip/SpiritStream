/**
 * Public api-client surface. The `api` object exposes every backend
 * namespace; each namespace lives in its own file under `./api/` and
 * shares the internal `fetchTypedJson` / `withConfirmToken` helpers in
 * `./api/_internal.ts`.
 *
 * Consumers do `import { api } from '@spiritstream/api-client'` and reach
 * for the namespaces they need (`api.profile.save`, `api.stream.start`,
 * `api.safety.panic`, …). The interface contract lives in
 * `./interface.ts` and is implemented by `./http-client.ts`.
 */

export type { FileBrowseResponse, FileHomeResponse, FileEntry } from '@spiritstream/types';
export type {
  EncoderPresetsResponse,
  ClientConfigResponse,
  RangeU32,
} from './api/system';

import { profile } from './api/profile';
import { stream } from './api/stream';
import { system } from './api/system';
import { settings } from './api/settings';
import { theme } from './api/theme';
import { obs } from './api/obs';
import { discord } from './api/discord';
import { chat } from './api/chat';
import { oauth } from './api/oauth';
import { files } from './api/files';
import { safety } from './api/safety';
import { audit } from './api/audit';
import { security } from './api/security';

/**
 * HTTP API wrapper that mirrors the SpiritStream typed REST surface.
 * All requests include credentials (cookies) for authentication.
 */
export const api = {
  profile,
  stream,
  system,
  settings,
  theme,
  obs,
  discord,
  chat,
  oauth,
  files,
  safety,
  audit,
  security,
};
