import { useState, useCallback, useRef } from 'react';
import { FileBrowserModal, type FileBrowserModalProps } from '@/components/modals/FileBrowserModal';
import type { DialogFilter } from '@/lib/backend/dialogTypes';

// Re-export for consumers who need the type
export type { FileBrowserModalProps };

interface FileBrowserState {
  open: boolean;
  mode: 'file' | 'directory' | 'save';
  title?: string;
  filters?: DialogFilter[];
  defaultFileName?: string;
  initialPath?: string;
}

interface FileBrowserOptions {
  title?: string;
  filters?: DialogFilter[];
  multiple?: boolean;
  initialPath?: string;
}

interface SaveFileOptions {
  title?: string;
  defaultPath?: string;
  filters?: DialogFilter[];
}

/**
 * Hook for using the FileBrowserModal with a promise-based API.
 * Returns the modal component to render and functions to open it.
 *
 * Usage:
 * ```tsx
 * function MyComponent() {
 *   const { FileBrowser, openFilePath, openDirectoryPath, saveFilePath } = useFileBrowser();
 *
 *   const handleBrowse = async () => {
 *     const path = await openFilePath({ filters: [{ name: 'JSON', extensions: ['json'] }] });
 *     if (path) {
 *       console.log('Selected:', path);
 *     }
 *   };
 *
 *   return (
 *     <>
 *       <button onClick={handleBrowse}>Browse</button>
 *       <FileBrowser />
 *     </>
 *   );
 * }
 * ```
 */
export function useFileBrowser() {
  const [state, setState] = useState<FileBrowserState>({
    open: false,
    mode: 'file',
  });

  // Store the resolve function for the current promise
  const resolveRef = useRef<((value: string | null) => void) | null>(null);

  // Handle selection
  const handleSelect = useCallback((path: string | null) => {
    if (resolveRef.current) {
      resolveRef.current(path);
      resolveRef.current = null;
    }
  }, []);

  // Handle close
  const handleClose = useCallback(() => {
    setState((s) => ({ ...s, open: false }));
    if (resolveRef.current) {
      resolveRef.current(null);
      resolveRef.current = null;
    }
  }, []);

  // Open file picker
  const openFilePath = useCallback(
    (options?: FileBrowserOptions): Promise<string | null> => {
      return new Promise((resolve) => {
        resolveRef.current = resolve;
        setState({
          open: true,
          mode: 'file',
          title: options?.title,
          filters: options?.filters,
          initialPath: options?.initialPath,
        });
      });
    },
    []
  );

  // Open directory picker
  const openDirectoryPath = useCallback(
    (options?: FileBrowserOptions): Promise<string | null> => {
      return new Promise((resolve) => {
        resolveRef.current = resolve;
        setState({
          open: true,
          mode: 'directory',
          title: options?.title,
          initialPath: options?.initialPath,
        });
      });
    },
    []
  );

  // Open save file dialog
  const saveFilePath = useCallback(
    (options?: SaveFileOptions): Promise<string | null> => {
      return new Promise((resolve) => {
        resolveRef.current = resolve;
        // Extract file name from defaultPath
        const defaultFileName = options?.defaultPath?.split('/').pop();
        setState({
          open: true,
          mode: 'save',
          title: options?.title,
          filters: options?.filters,
          defaultFileName,
        });
      });
    },
    []
  );

  // The modal component to render
  const FileBrowser = useCallback(
    () => (
      <FileBrowserModal
        open={state.open}
        onClose={handleClose}
        onSelect={handleSelect}
        mode={state.mode}
        title={state.title}
        filters={state.filters}
        defaultFileName={state.defaultFileName}
        initialPath={state.initialPath}
      />
    ),
    [state, handleClose, handleSelect]
  );

  return {
    FileBrowser,
    openFilePath,
    openDirectoryPath,
    saveFilePath,
  };
}
