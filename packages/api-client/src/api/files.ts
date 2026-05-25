import type { FileBrowseResponse, FileHomeResponse } from '@spiritstream/types';
import { fetchTypedJson } from './_internal';

export const files = {
  /** Browse a directory. Empty path returns server-side default (typically home). */
  browse: (path?: string) =>
    fetchTypedJson<FileBrowseResponse>(
      'GET',
      '/api/v1/files/browse',
      path ? { path } : undefined,
    ),
  /** Resolve the home directory for the current user. */
  home: () => fetchTypedJson<FileHomeResponse>('GET', '/api/v1/files/home'),
  /** Open a path in the OS file manager / default application. */
  open: (path: string) =>
    fetchTypedJson<void>('POST', '/api/v1/files/open', undefined, { path }),
};
