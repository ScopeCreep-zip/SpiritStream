#!/usr/bin/env node
/**
 * Assemble the Tauri 2 updater `latest.json` from the local `.sig`
 * files produced by `tauri signer sign` after a release build.
 *
 * Usage:
 *
 *   # Sign artifacts locally (one-time per release):
 *   for f in SpiritStream*.app.tar.gz SpiritStream*.AppImage \
 *            SpiritStream*.msi SpiritStream*.nsis.zip; do
 *     pnpm tauri signer sign \
 *       -k ~/.spiritstream-updater.key \
 *       -f "$f"
 *   done
 *
 *   # Build latest.json from the .sig files in the cwd:
 *   node scripts/build-updater-manifest.mjs \
 *     --version 1.2.2 \
 *     --release-tag v1.2.2 \
 *     --notes-file CHANGELOG.md \
 *     --out latest.json
 *
 * The script:
 *   1. Discovers `.sig` files in the cwd, classifies each by platform
 *      key (darwin-aarch64 / darwin-x86_64 / linux-x86_64 /
 *      windows-x86_64) from the artifact's filename.
 *   2. Reads each `.sig` body verbatim (Tauri-updater consumes the
 *      base64 minisign blob exactly as `tauri signer sign` emits it).
 *   3. Builds the GitHub Releases download URL for each artifact at
 *      `https://github.com/<owner>/<repo>/releases/download/<tag>/<file>`.
 *   4. Writes `latest.json` matching the Tauri-updater schema.
 *
 * Tauri-updater schema reference:
 *   https://v2.tauri.app/plugin/updater/#json-format
 *
 * Anything other than the four supported (platform, file-suffix)
 * combinations is logged + skipped without failing — the maintainer
 * can run the script on a partial set during canary releases.
 */

import { readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const OWNER_REPO = 'ScopeCreep-zip/SpiritStream';

// Per the Tauri updater docs, the platform key is `{os}-{arch}`. We
// classify each .sig by inspecting its underlying artifact's filename
// against the patterns `tauri-action` produces.
const PLATFORM_RULES = [
  // macOS Apple Silicon: tauri produces SpiritStream_1.2.2_aarch64.app.tar.gz
  { key: 'darwin-aarch64', test: (f) => /aarch64.*\.app\.tar\.gz$/i.test(f) },
  // macOS Intel
  { key: 'darwin-x86_64', test: (f) => /x64.*\.app\.tar\.gz$/i.test(f) || /x86_64.*\.app\.tar\.gz$/i.test(f) },
  // Windows: tauri produces SpiritStream_1.2.2_x64-setup.nsis.zip or .msi.zip
  { key: 'windows-x86_64', test: (f) => /\.(msi|nsis)\.zip$/i.test(f) || /\.exe$/i.test(f) },
  // Linux AppImage
  { key: 'linux-x86_64', test: (f) => /\.AppImage(\.tar\.gz)?$/i.test(f) },
];

function getArg(name, fallback) {
  const idx = process.argv.indexOf(name);
  if (idx === -1 || idx === process.argv.length - 1) return fallback;
  return process.argv[idx + 1];
}

function classify(filename) {
  for (const rule of PLATFORM_RULES) {
    if (rule.test(filename)) return rule.key;
  }
  return null;
}

function main() {
  const version = getArg('--version');
  const releaseTag = getArg('--release-tag', version ? `v${version}` : null);
  const notesFile = getArg('--notes-file');
  const outPath = getArg('--out', 'latest.json');
  const dir = getArg('--dir', '.');

  if (!version || !releaseTag) {
    console.error('Usage: build-updater-manifest.mjs --version <semver> [--release-tag vX.Y.Z] [--notes-file CHANGELOG.md] [--dir .] [--out latest.json]');
    process.exit(64); // EX_USAGE
  }

  const notes = notesFile ? readFileSync(notesFile, 'utf8').trim() : `Release ${releaseTag}`;
  const pubDate = new Date().toISOString();

  const platforms = {};
  const skipped = [];

  for (const entry of readdirSync(dir)) {
    if (!entry.endsWith('.sig')) continue;
    // Skip cosign sigs — they use the `.cosign.sig` suffix; the Tauri
    // updater sigs use the bare `.sig` suffix and are the only thing
    // we feed into latest.json. (Defense-in-depth — the cosign step
    // in release.yml already separates them.)
    if (entry.endsWith('.cosign.sig')) continue;

    const sigPath = join(dir, entry);
    const artifactName = entry.replace(/\.sig$/i, '');
    const artifactPath = join(dir, artifactName);

    try {
      statSync(artifactPath);
    } catch {
      skipped.push(`${entry} (artifact ${artifactName} missing in ${dir})`);
      continue;
    }

    const platform = classify(artifactName);
    if (!platform) {
      skipped.push(`${entry} (could not classify platform from ${artifactName})`);
      continue;
    }
    if (platforms[platform]) {
      skipped.push(`${entry} (duplicate for ${platform}; already have ${platforms[platform].url.split('/').pop()})`);
      continue;
    }

    const signature = readFileSync(sigPath, 'utf8').trim();
    platforms[platform] = {
      signature,
      url: `https://github.com/${OWNER_REPO}/releases/download/${releaseTag}/${encodeURIComponent(artifactName)}`,
    };
    console.log(`  ${platform}: ${artifactName}`);
  }

  if (Object.keys(platforms).length === 0) {
    console.error('No .sig files matched a supported platform. Did you run `tauri signer sign` first?');
    process.exit(65); // EX_DATAERR
  }

  const manifest = {
    version,
    notes,
    pub_date: pubDate,
    platforms,
  };

  const json = JSON.stringify(manifest, null, 2) + '\n';
  if (outPath === '-') {
    process.stdout.write(json);
  } else {
    writeFileSync(outPath, json);
    console.log(`\nWrote ${outPath} with ${Object.keys(platforms).length} platform(s).`);
  }

  if (skipped.length > 0) {
    console.warn('\nSkipped:');
    for (const s of skipped) console.warn(`  ${s}`);
  }
}

main();
