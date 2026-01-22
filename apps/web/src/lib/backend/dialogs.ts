import { backendMode } from './env';
import { dialogs as tauriDialogs } from './tauriDialogs';
import { dialogs as httpDialogs } from './httpDialogs';

export const dialogs = backendMode === 'tauri' ? tauriDialogs : httpDialogs;

export type { DialogFilter, OpenFileOptions, SaveFileOptions, OpenTextResult } from './dialogTypes';
