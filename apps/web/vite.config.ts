import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import path from 'path';

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],

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
    // Pre-split the heavy vendor surfaces so no single chunk dominates
    // load time. These 5 buckets cover ~70% of bundle weight and keep
    // `index.js` to first-paint code only — the alternative was Vite's
    // chunk-size warning firing on every build at ~1.15 MB.
    rollupOptions: {
      output: {
        manualChunks: {
          'vendor-react': ['react', 'react-dom', 'react-i18next', 'i18next'],
          'vendor-tauri': ['@tauri-apps/api'],
          'vendor-radix': ['@radix-ui/react-menubar'],
          'vendor-icons': ['lucide-react'],
          'vendor-state': ['zustand'],
        },
      },
    },
    // The main `index.js` is ~1 MB minified / ~295 kB gzipped after the
    // vendor splits — dominated by inlined i18n resources (11 locales)
    // and the chat platform connectors' UI. For a desktop-shipped Tauri
    // app + the gzip-over-localhost browser case there's no cold-load
    // concern at that size; the 1100 kB limit silences Vite's arbitrary
    // 500 kB default without losing the warning if the bundle balloons.
    chunkSizeWarningLimit: 1100,
  },

  // Environment variable prefixes
  envPrefix: ['VITE_'],
});
