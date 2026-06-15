import type { FileBrowseResponse, FileHomeResponse } from '@spiritstream/types';
import { filesBrowse, filesHome, filesOpen } from '../generated';

export const files = {
  /** Browse a directory. Empty path returns server-side default (typically home). */
  browse: async (path?: string): Promise<FileBrowseResponse> => {
    const { data } = await filesBrowse({
      query: path ? { path } : undefined,
      throwOnError: true,
    });
    return data as FileBrowseResponse;
  },
  /** Resolve the home directory for the current user. */
  home: async (): Promise<FileHomeResponse> => {
    const { data } = await filesHome({ throwOnError: true });
    return data as FileHomeResponse;
  },
  /** Open a path in the OS file manager / default application. */
  open: async (path: string): Promise<void> => {
    await filesOpen({ body: { path }, throwOnError: true });
  },
};
