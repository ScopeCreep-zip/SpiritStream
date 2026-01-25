// Check at runtime since Tauri globals may not be set at module load time
// Tauri 2.x uses __TAURI_INTERNALS__ instead of __TAURI__
export const isTauri = () =>
  Boolean((window as unknown as Record<string, unknown>).__TAURI_INTERNALS__);
