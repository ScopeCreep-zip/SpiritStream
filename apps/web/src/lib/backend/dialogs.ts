import { isTauri } from './env';
import { dialogs as tauriDialogs } from './tauriDialogs';
import { dialogs as httpDialogs } from './httpDialogs';
import type { OpenFileOptions, SaveFileOptions, OpenTextResult } from './dialogTypes';

// Dialogs should use Tauri native pickers when in Tauri environment,
// regardless of which backend mode is used for API calls.
// backendMode='http' is the default even in Tauri (for API calls),
// but we still want native file dialogs when running in Tauri webview.
//
// IMPORTANT: We use a lazy wrapper because window.__TAURI_INTERNALS__ may not be
// available at module evaluation time. Each method checks isTauri() at
// call time to ensure the correct implementation is used.
export const dialogs = {
  openFilePath: async (options?: OpenFileOptions): Promise<string | null> => {
    if (isTauri()) {
      return tauriDialogs.openFilePath(options);
    }
    // httpDialogs.openFilePath always returns null (browser sandbox limitation)
    return httpDialogs.openFilePath();
  },
  openTextFile: (options?: OpenFileOptions): Promise<OpenTextResult | null> => {
    if (isTauri()) {
      return tauriDialogs.openTextFile(options);
    }
    return httpDialogs.openTextFile(options);
  },
  openDirectoryPath: (options?: OpenFileOptions): Promise<string | null> => {
    if (isTauri()) {
      return tauriDialogs.openDirectoryPath(options);
    }
    return httpDialogs.openDirectoryPath();
  },
  saveFilePath: (options?: SaveFileOptions): Promise<string | null> => {
    if (isTauri()) {
      return tauriDialogs.saveFilePath(options);
    }
    return httpDialogs.saveFilePath();
  },
  saveTextFile: (options: SaveFileOptions & { content: string }): Promise<void> => {
    if (isTauri()) {
      return tauriDialogs.saveTextFile(options);
    }
    return httpDialogs.saveTextFile(options);
  },
  openExternal: (target: string): Promise<void> => {
    if (isTauri()) {
      return tauriDialogs.openExternal(target);
    }
    return httpDialogs.openExternal(target);
  },
};

export type { DialogFilter, OpenFileOptions, SaveFileOptions, OpenTextResult } from './dialogTypes';
