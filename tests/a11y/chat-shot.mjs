// Dev tool: snapshot the chat aside at various widths to iterate on layout.
// Usage: pnpm tsx tests/a11y/chat-shot.mjs [outdir]
//
// NOT a test — kept here so it has access to the workspace's installed
// Playwright + tsx without dragging deps into /tmp.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';

const outDir = resolve(process.argv[2] ?? '/tmp');
await mkdir(outDir, { recursive: true });

const PORT = 4174;
const previewCwd = resolve('apps/web');
const server = spawn('pnpm', ['exec', 'vite', 'preview', '--port', String(PORT), '--strictPort'], {
  cwd: previewCwd,
  stdio: 'inherit',
});

await new Promise((r) => setTimeout(r, 2500));

const browser = await chromium.launch();
try {
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  const page = await ctx.newPage();
  // Pin the backend URL to a guaranteed-closed port. The PROD bundle's
  // URL inference now uses the page origin (backend-served UI), which
  // here is the vite preview server — its SPA fallback would answer
  // /api/v1/ready with 200 + HTML and flip the app past the overlay
  // this audit expects. A closed port keeps the render deterministic.
  await page.addInitScript(() => {
    window.localStorage.setItem('spiritstream-backend-url', 'http://127.0.0.1:1');
  });
  await page.goto(`http://localhost:${PORT}/`, { waitUntil: 'networkidle' });
  await page.waitForTimeout(1500);

  const chat = page.locator('aside[aria-label="Chat"]');
  if (await chat.count()) {
    await chat.screenshot({ path: resolve(outDir, 'chat-aside.png') });
    console.log('saved', resolve(outDir, 'chat-aside.png'));
  } else {
    console.error('chat aside not found');
  }
} finally {
  await browser.close();
  server.kill();
}
