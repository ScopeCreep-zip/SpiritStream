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

export const dialogs = {
  openFilePath: async (): Promise<string | null> => null,
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
  openDirectoryPath: async (): Promise<string | null> => null,
  saveFilePath: async (): Promise<string | null> => null,
  saveTextFile: async (options: SaveFileOptions & { content: string }): Promise<void> => {
    if (typeof window === 'undefined') return;
    const name = options.defaultPath || 'spiritstream-export.txt';
    const blob = new Blob([options.content], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = name;
    document.body.appendChild(anchor);
    anchor.click();
    anchor.remove();
    URL.revokeObjectURL(url);
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
