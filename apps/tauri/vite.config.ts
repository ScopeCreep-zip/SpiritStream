import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import path from 'path';

// This config points to the apps/web source for desktop builds
export default defineConfig({
  plugins: [react(), tailwindcss()],

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
    target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
    minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    outDir: path.resolve(__dirname, 'dist'),
  },

  envPrefix: ['VITE_', 'TAURI_'],
});
