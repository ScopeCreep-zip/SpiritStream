import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import path from 'path';

/**
 * Plugin that enables cross-origin isolation for SharedArrayBuffer support.
 *
 * Sets three required headers on ALL responses (including worker modules):
 * - Cross-Origin-Opener-Policy: same-origin
 * - Cross-Origin-Embedder-Policy: credentialless (allows cross-origin iframes without CORP)
 * - Cross-Origin-Resource-Policy: cross-origin (allows worker module imports)
 *
 * Using middleware instead of server.headers ensures these apply to @fs/ paths
 * that Vite uses for worker module imports.
 *
 * References:
 * - https://web.dev/articles/coop-coep
 * - https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Cross-Origin-Embedder-Policy
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

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss(), crossOriginIsolation()],

  clearScreen: false,

  server: {
    port: 1420,
    strictPort: true,
  },

  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },

  build: {
    outDir: 'dist',
    emptyOutDir: true,
    // Modern browsers only
    target: 'esnext',
    minify: true,
    sourcemap: false,
    // Optimize chunk splitting for better caching and parallel loading
    rollupOptions: {
      output: {
        manualChunks: {
          // Vendor chunks - split heavy dependencies for better caching
          'vendor-react': ['react', 'react-dom', 'react-router-dom'],
          'vendor-ui': ['@dnd-kit/core', '@dnd-kit/sortable', '@dnd-kit/utilities', 'lucide-react'],
          'vendor-state': ['zustand', '@tanstack/react-query', 'i18next', 'react-i18next'],
          // Tauri API separate chunk (only loaded in desktop mode)
          'vendor-tauri': ['@tauri-apps/api'],
        },
      },
    },
    // CSS code splitting for async chunks
    cssCodeSplit: true,
  },

  // Environment variable prefixes
  envPrefix: ['VITE_'],
});
