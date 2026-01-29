export { api } from './api';
export { events, initBackendConnection } from './events';
export { dialogs } from './dialogs';
export {
  backendMode,
  backendUrlStorageKey,
  getBackendBaseUrl,
  getBackendWsUrl,
  updateBackendUrl,
  clearBackendUrl,
  checkAuth,
  login,
} from './env';
export type { WebRtcInfo } from '@/lib/tauri';
