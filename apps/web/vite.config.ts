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
  },

  // Environment variable prefixes
  envPrefix: ['VITE_'],
});
