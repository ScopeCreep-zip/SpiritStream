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
  },

  // Environment variable prefixes
  envPrefix: ['VITE_'],
});
