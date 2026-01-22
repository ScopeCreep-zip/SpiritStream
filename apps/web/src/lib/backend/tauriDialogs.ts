import { open, save } from '@tauri-apps/plugin-dialog';
import { readTextFile, writeTextFile } from '@tauri-apps/plugin-fs';
import { open as openPath } from '@tauri-apps/plugin-shell';
import type { OpenFileOptions, SaveFileOptions, OpenTextResult } from './dialogTypes';

function normalizePath(result: string | string[] | null): string | null {
  if (!result) return null;
  if (typeof result === 'string') return result;
  return result.length > 0 ? result[0] : null;
}

export const dialogs = {
  openFilePath: async (options?: OpenFileOptions): Promise<string | null> => {
    const selected = await open(options);
    return normalizePath(selected);
  },
  openTextFile: async (options?: OpenFileOptions): Promise<OpenTextResult | null> => {
    const selected = await open(options);
    const path = normalizePath(selected);
    if (!path) return null;
    const content = await readTextFile(path);
    const name = path.split(/[/\\]/).pop() || 'file';
    return { name, content };
  },
  openDirectoryPath: async (options?: OpenFileOptions): Promise<string | null> => {
    const selected = await open({ ...options, directory: true, multiple: false });
    return normalizePath(selected);
  },
  saveFilePath: async (options?: SaveFileOptions): Promise<string | null> => {
    const selected = await save(options);
    return selected || null;
  },
  saveTextFile: async (options: SaveFileOptions & { content: string }): Promise<void> => {
    const selected = await save(options);
    if (!selected) return;
    await writeTextFile(selected, options.content);
  },
  openExternal: async (target: string): Promise<void> => {
    await openPath(target);
  },
};
