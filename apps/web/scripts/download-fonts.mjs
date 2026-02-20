#!/usr/bin/env node
/**
 * Download Google Fonts (Space Grotesk + JetBrains Mono) as local woff2 files.
 *
 * Usage: node apps/web/scripts/download-fonts.mjs
 *
 * This fetches the Google Fonts CSS (with a Chrome User-Agent to get woff2 variable fonts),
 * parses out the @font-face blocks, and downloads each woff2 file with a name matching
 * the @font-face declarations in globals.css.
 *
 * Expected output files:
 *   public/fonts/space-grotesk-300-700-latin.woff2
 *   public/fonts/space-grotesk-300-700-latin-ext.woff2
 *   public/fonts/space-grotesk-300-700-vietnamese.woff2
 *   public/fonts/jetbrains-mono-100-800-latin.woff2
 *   public/fonts/jetbrains-mono-100-800-latin-ext.woff2
 *   public/fonts/jetbrains-mono-100-800-cyrillic.woff2
 *   public/fonts/jetbrains-mono-100-800-cyrillic-ext.woff2
 *   public/fonts/jetbrains-mono-100-800-greek.woff2
 *   public/fonts/jetbrains-mono-100-800-vietnamese.woff2
 */

import https from 'node:https';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const FONT_DIR = path.resolve(__dirname, '../public/fonts');

const GOOGLE_FONTS_URL =
  'https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap';

function fetchBuffer(url) {
  return new Promise((resolve, reject) => {
    https.get(
      url,
      {
        headers: {
          'User-Agent':
            'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
        },
      },
      (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          return fetchBuffer(res.headers.location).then(resolve, reject);
        }
        if (res.statusCode !== 200) {
          return reject(new Error(`HTTP ${res.statusCode} for ${url}`));
        }
        const chunks = [];
        res.on('data', (chunk) => chunks.push(chunk));
        res.on('end', () => resolve(Buffer.concat(chunks)));
        res.on('error', reject);
      }
    ).on('error', reject);
  });
}

async function main() {
  fs.mkdirSync(FONT_DIR, { recursive: true });

  console.log('Fetching Google Fonts CSS...\n');
  const css = (await fetchBuffer(GOOGLE_FONTS_URL)).toString('utf-8');

  // Parse @font-face blocks (subset comment precedes each block)
  const faceRegex = /\/\*\s*([a-z-]+)\s*\*\/\s*@font-face\s*\{([^}]+)\}/g;
  const faces = [];
  let match;
  while ((match = faceRegex.exec(css)) !== null) {
    const subset = match[1];
    const block = match[2];
    const family = block.match(/font-family:\s*'([^']+)'/)?.[1];
    const style = block.match(/font-style:\s*(\w+)/)?.[1] || 'normal';
    const weight = block.match(/font-weight:\s*([^;]+)/)?.[1]?.trim();
    const url = block.match(/url\((https:\/\/[^)]+\.woff2)\)/)?.[1];
    const range = block.match(/unicode-range:\s*([^;]+)/)?.[1]?.trim();
    if (family && weight && url) {
      faces.push({ family, style, weight, url, range, subset });
    }
  }

  if (faces.length === 0) {
    console.error('ERROR: No @font-face blocks found. Google might have changed their API.\n');
    console.error('Raw CSS:\n');
    console.error(css);
    process.exit(1);
  }

  console.log(`Found ${faces.length} @font-face declarations.\n`);

  // Download each file with a name matching our globals.css @font-face src paths
  for (const face of faces) {
    const safeName = face.family.toLowerCase().replace(/\s+/g, '-');
    const weightSlug = face.weight.replace(/\s+/g, '-'); // e.g. "300 700" -> "300-700"
    const filename = `${safeName}-${weightSlug}-${face.subset}.woff2`;
    const filepath = path.join(FONT_DIR, filename);

    process.stdout.write(`  ${filename} ...`);
    const data = await fetchBuffer(face.url);
    fs.writeFileSync(filepath, data);
    console.log(` ${(data.length / 1024).toFixed(1)} KB`);
  }

  // Summary
  console.log('');
  const files = fs.readdirSync(FONT_DIR).filter((f) => f.endsWith('.woff2'));
  const totalSize = files.reduce(
    (sum, f) => sum + fs.statSync(path.join(FONT_DIR, f)).size,
    0
  );
  console.log(`Downloaded ${files.length} files (${(totalSize / 1024).toFixed(0)} KB total)`);
  console.log(`Location: ${FONT_DIR}/`);
  console.log('\nDone! Fonts are ready for local serving.');
}

main().catch((err) => {
  console.error('Error:', err);
  process.exit(1);
});
