/// <reference types="vitest" />
import { defineConfig } from 'vitest/config';

// N5: api-client coverage gate. Scoped to the three logic-bearing
// modules — URL resolution + localhost retry (`config.ts`), the WebSocket
// event bus (`events.ts`), and the typed-REST error mapping
// (`api/_internal.ts`). The per-namespace files under `api/*.ts` are thin
// passthroughs over `fetchTypedJson`; their shared behaviour is covered
// here, and the generated `@hey-api` client + pure type-declaration files
// carry no branches worth pinning. Ratchet thresholds sit a few points
// below measured so a regression fails CI while the plan floor
// (≥50% lines) stays comfortably met.
export default defineConfig({
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test-setup.ts'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'lcov'],
      include: ['src/config.ts', 'src/events.ts', 'src/api/_internal.ts'],
      thresholds: {
        lines: 80,
        functions: 80,
        branches: 62,
        statements: 80,
      },
    },
  },
});
