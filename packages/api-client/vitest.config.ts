/// <reference types="vitest" />
import { defineConfig } from 'vitest/config';

// N5: api-client coverage gate. Scoped to the logic-bearing modules — URL
// resolution + localhost/Request-aware retry (`config.ts`), the WebSocket
// event bus (`events.ts`), the structured non-2xx error mapping the generated
// client's response interceptor uses (`clientConfig.ts`), and the
// untrusted-inbound chat-message guard (`validation/chatMessage.ts`). The
// per-namespace files under `api/*.ts` are thin facades over the generated
// `@hey-api` SDK; the generated client + pure type-declaration files carry no
// branches worth pinning. Ratchet thresholds sit a few points below measured
// so a regression fails CI while the plan floor (≥50% lines) stays met.
export default defineConfig({
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test-setup.ts'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'lcov'],
      include: [
        'src/config.ts',
        'src/events.ts',
        'src/clientConfig.ts',
        'src/validation/chatMessage.ts',
      ],
      thresholds: {
        lines: 80,
        functions: 80,
        branches: 62,
        statements: 80,
      },
    },
  },
});
