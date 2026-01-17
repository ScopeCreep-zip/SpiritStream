import { backendMode } from './env';
import { api as tauriApi } from './tauriApi';
import { api as httpApi } from './httpApi';

export const api = backendMode === 'tauri' ? tauriApi : httpApi;
