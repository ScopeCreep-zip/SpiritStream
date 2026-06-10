#!/usr/bin/env node
/**
 * i18n drift gate.
 *
 * Checks three invariants and exits non-zero when any is violated:
 *   1. Every static `t('key', …)` / `i18n.t('key', …)` key used in
 *      `apps/web/src` and `packages/ui/src` exists in `en.json`.
 *   2. Every key in `en.json` exists in every other locale file
 *      (missing keys fall back to English silently at runtime —
 *      exactly the drift this gate exists to catch).
 *   3. No locale file carries keys that `en.json` doesn't have
 *      (orphans rot forever because nothing ever reads them).
 *
 * Dynamic keys (template literals like `t(\`chat.platforms.\${p}\`)`)
 * can't be resolved statically; their known prefixes are declared in
 * DYNAMIC_PREFIXES and every en.json key under such a prefix is
 * treated as used.
 */

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
const localesDir = join(repoRoot, 'apps/web/src/locales');
const SOURCE_ROOTS = [join(repoRoot, 'apps/web/src'), join(repoRoot, 'packages/ui/src')];

// Prefixes used through template-literal keys somewhere in the code.
const DYNAMIC_PREFIXES = ['chat.platforms.', 'connection.', 'errors.'];

// ---------------------------------------------------------------------------

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    if (name === 'node_modules' || name === 'generated' || name === 'locales') continue;
    const full = join(dir, name);
    const st = statSync(full);
    if (st.isDirectory()) walk(full, out);
    else if (/\.(ts|tsx)$/.test(name) && !/\.test\.(ts|tsx)$/.test(name)) out.push(full);
  }
  return out;
}

function flatten(obj, prefix = '', out = new Set()) {
  for (const [k, v] of Object.entries(obj)) {
    const key = prefix ? `${prefix}.${k}` : k;
    if (v && typeof v === 'object') flatten(v, key, out);
    else out.add(key);
  }
  return out;
}

// Match t('key' / t("key" / i18n.t('key' — first argument only, static
// string literals. The callee must be exactly `t` (or `.t`) so helper
// calls like `format('x')` don't false-positive.
const KEY_RE = /(?<![\w$])(?:i18n\.)?t\(\s*(['"])((?:(?!\1).)+)\1/g;

const usedKeys = new Set();
for (const root of SOURCE_ROOTS) {
  for (const file of walk(root)) {
    const text = readFileSync(file, 'utf8');
    for (const match of text.matchAll(KEY_RE)) {
      usedKeys.add(match[2]);
    }
  }
}

const en = JSON.parse(readFileSync(join(localesDir, 'en.json'), 'utf8'));
const enKeys = flatten(en);

const locales = readdirSync(localesDir)
  .filter((f) => f.endsWith('.json') && f !== 'en.json')
  .sort();

let failed = false;
const fail = (header, items) => {
  if (items.length === 0) return;
  failed = true;
  console.error(`\n${header} (${items.length}):`);
  for (const item of items.sort()) console.error(`  - ${item}`);
};

// 1. Code → en.json
const missingFromEn = [...usedKeys].filter((k) => !enKeys.has(k));
fail('Keys used in code but missing from en.json', missingFromEn);

// 2 + 3. en.json ↔ every locale
for (const file of locales) {
  const data = JSON.parse(readFileSync(join(localesDir, file), 'utf8'));
  const keys = flatten(data);
  fail(`Keys missing from ${file}`, [...enKeys].filter((k) => !keys.has(k)));
  fail(`Orphan keys in ${file} (not in en.json)`, [...keys].filter((k) => !enKeys.has(k)));
}

// Report unused en.json keys as a warning only — deleting them is a
// judgment call (some are read by upcoming code), so they don't gate.
const dynamicCovered = (k) => DYNAMIC_PREFIXES.some((p) => k.startsWith(p));
const unused = [...enKeys].filter((k) => !usedKeys.has(k) && !dynamicCovered(k));
if (unused.length > 0) {
  console.warn(`\nWarning: ${unused.length} en.json keys have no static t() usage (not gating).`);
}

if (failed) {
  console.error('\ni18n gate failed.');
  process.exit(1);
}
console.log(`i18n gate OK: ${usedKeys.size} used keys, ${enKeys.size} en keys, ${locales.length} locales in sync.`);
