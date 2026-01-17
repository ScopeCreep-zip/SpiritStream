import { backendMode } from './env';
import { events as tauriEvents } from './tauriEvents';
import { events as httpEvents } from './httpEvents';

export const events = backendMode === 'tauri' ? tauriEvents : httpEvents;
