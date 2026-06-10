/// <reference types="vitest" />
import { defineConfig, type Plugin } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import path from 'path';

// Production Content-Security-Policy with NO `'unsafe-inline'` anywhere.
// `script-src 'self'`: the only formerly-inline script (theme bootstrap) is now
// served from /theme-init.js. `style-src 'self'`: the boot <style> moved to
// /boot.css and custom-theme tokens apply via CSSOM `setProperty` (CSP-exempt),
// so no inline <style> remains. `https://fonts.googleapis.com` is kept for the
// Google Fonts stylesheet <link> (the HTTP transport header omits it on purpose
// for the privacy-strict Docker path; see crates/transport-http/src/lib.rs).
const PROD_CSP =
  "default-src 'self'; script-src 'self'; " +
  "style-src 'self' https://fonts.googleapis.com; " +
  "font-src 'self' https://fonts.gstatic.com; img-src 'self' data:; " +
  'connect-src \'self\' ipc: http://ipc.localhost http://127.0.0.1:* ' +
  'http://localhost:* ws://127.0.0.1:* ws://localhost:* ' +
  'https://*.spiritstream.io wss://*.spiritstream.io';

// Inject the strict CSP <meta> at build only. Dev is intentionally left
// CSP-free so Vite's inline React-refresh preamble (HMR) isn't blocked; a
// static strict meta in index.html would break both web and Tauri dev.
function cspMetaInjection(): Plugin {
  return {
    name: 'spiritstream-csp-meta',
    apply: 'build',
    transformIndexHtml() {
      return [
        {
          tag: 'meta',
          attrs: { 'http-equiv': 'Content-Security-Policy', content: PROD_CSP },
          injectTo: 'head-prepend',
        },
      ];
    },
  };
}

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss(), cspMetaInjection()],

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

  // N3: Vitest config. Test files are colocated next to source as
  // `*.test.ts` / `*.test.tsx` so a refactor that moves the source
  // automatically picks up the test. jsdom env mirrors the browser
  // tree for React-Testing-Library `render()` calls; node env (the
  // default) suits pure-logic store / helper tests.
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test-setup.ts'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'lcov'],
      // N5 scopes the gate to the Zustand store layer — the frontend's
      // only behaviour worth pinning (UI orchestration; all business
      // logic lives in crates/core). Components/hooks/views are render
      // glue exercised by the a11y + integration suites, not unit
      // coverage. Widening this include to `src/**` would re-introduce
      // an unsatisfiable gate (the bulk of `src/` is untested view code).
      include: ['src/stores/**/*.{ts,tsx}'],
      exclude: [
        'src/**/*.test.{ts,tsx}',
        'src/**/__tests__/**',
      ],
      // N5 coverage gate. Ratchet baselines a few points below the
      // measured store coverage so a regression fails CI, while the
      // plan floor (≥50% lines on the store layer) stays comfortably
      // met. Lowering needs an explicit "we removed test surface" PR.
      thresholds: {
        lines: 75,
        functions: 75,
        branches: 70,
        statements: 75,
      },
    },
  },

  build: {
    outDir: 'dist',
    emptyOutDir: true,
    // Modern browsers only
    target: 'esnext',
    minify: true,
    sourcemap: false,
    // Pre-split heavy vendor surfaces so no single chunk dominates
    // load time. The function form (vs the object form) lets us bucket
    // any package whose path matches a pattern, which is necessary for
    // the @tauri-apps/plugin-* family (8 plugins, individually small,
    // collectively ~400 kB) without listing each one. Goal: keep the
    // main `index.js` under 500 kB so Vite's chunk-size warning stops
    // firing on every build.
    rollupOptions: {
      output: {
        manualChunks(id) {
          // Match `/node_modules/<pkg>` so we only bucket actual
          // package roots — substring matches like `id.includes('react/')`
          // catch unrelated paths inside other vendors and create the
          // circular-chunk warnings Rollup emits.
          const m = id.match(
            /node_modules\/(?:\.pnpm\/[^/]+\/node_modules\/)?(@[^/]+\/[^/]+|[^/]+)/
          );
          if (m) {
            const pkg = m[1];
            // React core only — keep the hydration path in one
            // chunk without pulling in i18next-family transitives
            // (those import small utils that Rollup buckets into
            // vendor-misc, creating a misc → react → misc cycle).
            if (pkg === 'react' || pkg === 'react-dom' || pkg === 'scheduler') {
              return 'vendor-react';
            }
            if (
              pkg === 'i18next' ||
              pkg.startsWith('i18next-') ||
              pkg.startsWith('react-i18next')
            ) {
              return 'vendor-i18n';
            }
            if (pkg.startsWith('@tauri-apps/plugin-')) return 'vendor-tauri-plugins';
            if (pkg.startsWith('@tauri-apps/')) return 'vendor-tauri';
            if (pkg.startsWith('@radix-ui/')) return 'vendor-radix';
            if (pkg === 'lucide-react') return 'vendor-icons';
            if (pkg === 'zustand') return 'vendor-state';
            if (pkg.startsWith('@tanstack/')) return 'vendor-query';
            if (pkg === 'clsx' || pkg === 'tailwind-merge') return 'vendor-classnames';
            // Everything else lands in vendor-misc. Keeping this as
            // an explicit branch (rather than the unconditional
            // fall-through that was producing circular chunks)
            // ensures only matched packages get bucketed and
            // app/internal modules continue to feed `index.js`.
            return 'vendor-misc';
          }
          // App code: keep the platform catalog (824 LOC of data) and
          // locale JSON out of the main entry. They're imported at
          // runtime in a single place but their size dominates the
          // bundle if left in `index.js`.
          if (id.includes('/types/generated-platforms')) {
            return 'app-platforms';
          }
          if (id.includes('/locales/')) {
            return 'app-locales';
          }
          return undefined;
        },
      },
    },
  },

  // Environment variable prefixes
  envPrefix: ['VITE_'],
});
