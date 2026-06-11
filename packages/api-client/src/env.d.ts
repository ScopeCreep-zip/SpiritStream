/**
 * Ambient type augmentations used by the api-client.
 *
 * The api-client is consumed from a Vite-built frontend; declaring the
 * minimal `import.meta.env` shape and the File System Access API (used for
 * file dialogs) here keeps the package self-typing.
 */

interface ImportMetaEnv {
  readonly VITE_BACKEND_MODE?: 'tauri' | 'http';
  readonly VITE_BACKEND_URL?: string;
  readonly VITE_BACKEND_WS_URL?: string;
  /** Vite built-ins: true for production builds / dev serve respectively. */
  readonly PROD: boolean;
  readonly DEV: boolean;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

// File System Access API (`window.showOpenFilePicker` / `showSaveFilePicker`).
// Standardized but not yet in TypeScript's default DOM lib.
interface ShowFilePickerOptions {
  excludeAcceptAllOption?: boolean;
  id?: string;
  startIn?: string;
  types?: { description?: string; accept: Record<string, string[]> }[];
  suggestedName?: string;
  multiple?: boolean;
}

interface FileSystemFileHandle {
  readonly kind: 'file';
  readonly name: string;
  getFile(): Promise<File>;
  createWritable(): Promise<FileSystemWritableFileStream>;
}

interface FileSystemWritableFileStream extends WritableStream {
  write(data: BufferSource | Blob | string): Promise<void>;
  close(): Promise<void>;
}

interface Window {
  showOpenFilePicker?(options?: ShowFilePickerOptions): Promise<FileSystemFileHandle[]>;
  showSaveFilePicker?(options?: ShowFilePickerOptions): Promise<FileSystemFileHandle>;
}
