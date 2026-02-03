import type { OpenFileOptions, SaveFileOptions, OpenTextResult, DialogFilter } from './dialogTypes';
import { getBackendBaseUrl, getAuthHeaders, safeFetch } from './env';

type PickerAcceptType = { description?: string; accept: Record<string, string[]> };

function buildAccept(filters?: DialogFilter[]): string | undefined {
  if (!filters || filters.length === 0) return undefined;
  const extensions = filters.flatMap((filter) => filter.extensions);
  const unique = Array.from(new Set(extensions));
  return unique.map((ext) => (ext.startsWith('.') ? ext : `.${ext}`)).join(',');
}

function buildPickerTypes(filters?: DialogFilter[]): PickerAcceptType[] | undefined {
  if (!filters || filters.length === 0) return undefined;
  return filters.map((filter) => ({
    description: filter.name,
    accept: {
      'application/octet-stream': filter.extensions.map((ext) =>
        ext.startsWith('.') ? ext : `.${ext}`
      ),
    },
  }));
}

async function openFileViaInput(options?: OpenFileOptions): Promise<File | null> {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    if (options?.multiple) {
      input.multiple = true;
    }
    const accept = buildAccept(options?.filters);
    if (accept) {
      input.accept = accept;
    }
    input.addEventListener('change', () => {
      const file = input.files && input.files.length > 0 ? input.files[0] : null;
      resolve(file);
    });
    input.click();
  });
}

/**
 * HTTP mode dialog implementations.
 *
 * In HTTP mode (browser context), native file path access is restricted
 * by browser security sandbox. Methods that would return file paths
 * return null instead. Use the text-based alternatives that work with
 * the browser File API.
 *
 * For operations requiring actual file paths (e.g., opening folders),
 * the dialogs.openExternal() method uses a server endpoint to perform
 * the operation on the backend.
 */
export const dialogs = {
  /**
   * Returns null in HTTP mode.
   *
   * Browser security sandbox prevents accessing native file paths.
   * Use openTextFile() instead, which uses the browser File API
   * to read file contents without exposing the path.
   */
  openFilePath: async (): Promise<string | null> => null,

  /**
   * Open a file and read its text content using browser File API.
   * Works in HTTP mode unlike openFilePath().
   */
  openTextFile: async (options?: OpenFileOptions): Promise<OpenTextResult | null> => {
    if (typeof window === 'undefined') return null;

    if (window.showOpenFilePicker) {
      try {
        const handles = await window.showOpenFilePicker({
          multiple: false,
          types: buildPickerTypes(options?.filters),
        });
        const file = await handles[0].getFile();
        return { name: file.name, content: await file.text() };
      } catch (error) {
        if ((error as Error).name === 'AbortError') {
          return null;
        }
        throw error;
      }
    }

    const file = await openFileViaInput(options);
    if (!file) return null;
    return { name: file.name, content: await file.text() };
  },
  /**
   * Returns null in HTTP mode.
   *
   * Browser security prevents selecting directories and exposing paths.
   * For opening a directory in the file manager, use openExternal()
   * which delegates to the backend server.
   */
  openDirectoryPath: async (): Promise<string | null> => null,

  /**
   * Returns null in HTTP mode.
   *
   * Browser security prevents accessing native save paths.
   * Use saveTextFile() instead, which triggers a browser download.
   */
  saveFilePath: async (): Promise<string | null> => null,

  /**
   * Save text content to a file using browser download.
   * Uses File System Access API when available to show save dialog every time.
   * Falls back to automatic download for older browsers.
   */
  saveTextFile: async (options: SaveFileOptions & { content: string }): Promise<void> => {
    if (typeof window === 'undefined') return;
    const name = options.defaultPath || 'spiritstream-export.txt';

    // Use File System Access API if available (shows save dialog every time).
    // Browser support: Chromium-based browsers 86+ only. Not supported in Firefox or Safari,
    // which will always use the legacy download fallback below.
    if (window.showSaveFilePicker) {
      try {
        const types = buildPickerTypes(options.filters);
        const handle = await window.showSaveFilePicker({
          suggestedName: name,
          types,
        });
        const writable = await handle.createWritable();
        await writable.write(options.content);
        await writable.close();
        return;
      } catch (error) {
        // User cancelled or browser doesn't support the API
        if ((error as Error).name === 'AbortError') {
          throw error; // Re-throw to indicate user cancellation
        }
        console.warn('[dialogs] File System Access API failed, falling back to download:', error);
        // Fall through to legacy download method
      }
    }

    // Fallback: Use legacy download method (auto-downloads to default folder)
    const blob = new Blob([options.content], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = name;
    document.body.appendChild(anchor);
    anchor.click();

    // Clean up after a short delay to ensure download starts
    setTimeout(() => {
      anchor.remove();
      URL.revokeObjectURL(url);
    }, 100);
  },
  openExternal: async (target: string): Promise<void> => {
    if (typeof window === 'undefined') return;

    // Check if this is a local file path (starts with / or looks like a Windows path)
    const isLocalPath =
      target.startsWith('/') ||
      /^[A-Za-z]:[/\\]/.test(target) ||
      target.startsWith('~');

    if (isLocalPath) {
      // Use server's file open endpoint for local paths
      try {
        const baseUrl = getBackendBaseUrl();
        const response = await safeFetch(`${baseUrl}/api/files/open`, {
          method: 'POST',
          headers: {
            ...getAuthHeaders(),
            'Content-Type': 'application/json',
          },
          credentials: 'include',
          body: JSON.stringify({ path: target }),
        });

        if (!response.ok) {
          const error = await response.text();
          console.error('[dialogs] Failed to open path:', error);
        }
      } catch (error) {
        console.error('[dialogs] Failed to open path:', error);
      }
    } else {
      // Use window.open for URLs
      window.open(target, '_blank');
    }
  },
};
