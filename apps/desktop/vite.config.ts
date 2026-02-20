import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import path from 'path';

/**
 * Plugin that enables cross-origin isolation for SharedArrayBuffer support.
 * Required for audio meter workers using SharedArrayBuffer in Tauri's WKWebView.
 */
function crossOriginIsolation() {
  return {
    name: 'cross-origin-isolation',
    configureServer(server: { middlewares: { use: (fn: (req: unknown, res: { setHeader: (name: string, value: string) => void }, next: () => void) => void) => void } }) {
      server.middlewares.use((_req, res, next) => {
        res.setHeader('Cross-Origin-Opener-Policy', 'same-origin');
        res.setHeader('Cross-Origin-Embedder-Policy', 'credentialless');
        res.setHeader('Cross-Origin-Resource-Policy', 'cross-origin');
        next();
      });
    },
  };
}

// This config points to the apps/web source for desktop builds
export default defineConfig({
  plugins: [react(), tailwindcss(), crossOriginIsolation()],

  clearScreen: false,

  root: path.resolve(__dirname, '../web'),

  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },

  resolve: {
    alias: {
      '@': path.resolve(__dirname, '../web/src'),
    },
  },

  build: {
    // safari17 matches macOS 14+ WKWebView — no polyfills for features native to the runtime
    target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome105' : 'safari17',
    minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    outDir: path.resolve(__dirname, '../web/dist'),
  },

  envPrefix: ['VITE_', 'TAURI_'],
});
