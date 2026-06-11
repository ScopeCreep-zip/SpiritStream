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
    // OS-negotiated by scripts/dev-desktop.ts for `pnpm dev:desktop`;
    // the Tauri-conventional 1420 only applies to a standalone `vite`.
    // strictPort stays: if the negotiated port got raced, fail loud.
    port: Number(process.env.SPIRITSTREAM_WEB_PORT ?? 1420),
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
