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
    headers: {
      // Required for SharedArrayBuffer support in Web Workers
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'require-corp',
    },
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
