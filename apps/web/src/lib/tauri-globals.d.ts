// TypeScript declaration for the Tauri global injected by the Tauri runtime
interface Window {
  __TAURI__?: unknown;
}

declare var window: Window;
