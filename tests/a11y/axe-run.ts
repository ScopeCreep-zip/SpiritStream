/**
 * axe-core a11y test against the built frontend.
 *
 * Boots a static file server on the bundle, launches headless
 * Chromium via Playwright, navigates the SPA, and runs axe-core
 * across each `/route` we care about. Fails the CI job on any
 * `serious` or `critical` violation.
 *
 * Usage: `pnpm tsx tests/a11y/axe-run.ts <dist-dir>`
 *
 * The script intentionally does NOT spin up the backend — we test
 * the rendered DOM at app-shell level (sidebar, modals, focus
 * affordances, color contrast). Route-specific data-driven views
 * that require a live API are tested in the frontend's own e2e
 * suite when one lands.
 */
import { spawn } from 'node:child_process';
import { resolve as resolvePath } from 'node:path';
import { existsSync } from 'node:fs';

import { chromium, type Browser, type Page } from '@playwright/test';
import { AxeBuilder } from '@axe-core/playwright';

const distArg = process.argv[2];
if (!distArg) {
  console.error('usage: axe-run.ts <dist-dir>');
  process.exit(2);
}
const distDir = resolvePath(distArg);
if (!existsSync(distDir)) {
  console.error(`dist dir does not exist: ${distDir}`);
  process.exit(2);
}

// Plain-Node static file server. Avoids dragging in another dep
// (serve, http-server) just to host one bundle.
const PORT = 4173;
const server = spawn('pnpm', ['exec', 'vite', 'preview', '--port', String(PORT), '--strictPort'], {
  cwd: distDir.replace(/\/dist$/, ''),
  stdio: 'inherit',
});

const ROUTES_TO_AUDIT = ['/'];
// Rules to ignore — these are intentional choices in SpiritStream's
// design system that axe-core flags as "needs review" but that we've
// confirmed pass WCAG 2.2 AA. Keep this list short and reviewed.
const IGNORED_RULES: string[] = [
  // None today. When adding an entry, link to the design-decision PR
  // and re-verify on every theme update.
];

let exitCode = 0;
let browser: Browser | null = null;

async function audit(page: Page, route: string): Promise<void> {
  await page.goto(`http://localhost:${PORT}${route}`, { waitUntil: 'networkidle' });
  const results = await new AxeBuilder({ page })
    .disableRules(IGNORED_RULES)
    .withTags(['wcag2a', 'wcag2aa', 'wcag22aa'])
    .analyze();

  const blocking = results.violations.filter(
    (v) => v.impact === 'serious' || v.impact === 'critical'
  );
  if (blocking.length === 0) {
    console.log(`[axe] ${route}: clean`);
    return;
  }
  exitCode = 1;
  for (const v of blocking) {
    console.error(`[axe] ${route}: ${v.id} (${v.impact}) — ${v.help}`);
    for (const node of v.nodes) {
      console.error(`        target: ${node.target.join(' ')}`);
    }
  }
}

async function waitForServer(): Promise<void> {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    try {
      const r = await fetch(`http://localhost:${PORT}/`);
      if (r.ok) return;
    } catch {
      // not ready yet
    }
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error('static server did not come up within 30s');
}

try {
  await waitForServer();
  browser = await chromium.launch();
  const ctx = await browser.newContext();
  const page = await ctx.newPage();
  for (const route of ROUTES_TO_AUDIT) {
    await audit(page, route);
  }
} catch (err) {
  console.error('[axe] fatal:', err);
  exitCode = 1;
} finally {
  if (browser) await browser.close();
  server.kill('SIGTERM');
  process.exit(exitCode);
}
