#!/usr/bin/env tsx
/**
 * Fetch the FFmpeg source tarball matching the pinned version in
 * `scripts/ffmpeg-pins.json` and place it in the cwd, ready for the
 * maintainer to `gh release upload` alongside the binary artifacts.
 *
 * Why: SpiritStream bundles GPL-licensed FFmpeg builds (the BtbN
 * Windows / Linux GPL variants are needed for NVENC / QSV / AMF
 * hardware encoders, and evermeet macOS is GPL by default). The GPL
 * requires the corresponding source code be available alongside the
 * binary distribution. Attaching the tarball to the same GitHub
 * release as the binaries satisfies the "machine-readable, accompanies
 * the binary" requirement; the in-app Third-Party Licenses page
 * displays the matching version + provides the GitHub Releases link.
 *
 * Usage:
 *
 *   pnpm tsx scripts/attach-ffmpeg-source.ts
 *
 *   # Custom output path:
 *   pnpm tsx scripts/attach-ffmpeg-source.ts --out ./staging/ffmpeg-source.tar.bz2
 *
 * The script:
 *   1. Reads `ffmpegVersion` from `scripts/ffmpeg-pins.json`.
 *   2. Downloads `https://ffmpeg.org/releases/ffmpeg-<version>.tar.bz2`
 *      — ffmpeg.org's canonical source distribution URL.
 *   3. Verifies file size (sanity check; ffmpeg.org doesn't publish a
 *      pinnable SHA-256 alongside the tarball, but signed releases
 *      are GPG-signed — verified by the bump-ffmpeg-pin helper).
 *   4. Writes to the cwd or `--out` path.
 *
 * Source URL is what ffmpeg.org itself links to from its Downloads
 * page — same authority anchor as the binary pins.
 */

import { execFileSync } from 'node:child_process';
import { readFileSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

interface PinFile {
  ffmpegVersion: string;
  _?: string;
}

function getArg(name: string, fallback: string): string {
  const idx = process.argv.indexOf(name);
  if (idx === -1 || idx === process.argv.length - 1) return fallback;
  return process.argv[idx + 1];
}

function main(): void {
  const pinsPath = join(__dirname, 'ffmpeg-pins.json');
  const pins = JSON.parse(readFileSync(pinsPath, 'utf8')) as PinFile;
  const version = pins.ffmpegVersion;
  if (!version) {
    console.error('ffmpegVersion missing from ffmpeg-pins.json');
    process.exit(65); // EX_DATAERR
  }

  const url = `https://ffmpeg.org/releases/ffmpeg-${version}.tar.bz2`;
  const outDefault = `ffmpeg-${version}.tar.bz2`;
  const outPath = getArg('--out', outDefault);

  console.log(`Fetching FFmpeg ${version} source: ${url}`);
  execFileSync('curl', ['-fLso', outPath, '--retry', '3', '--retry-delay', '2', url], {
    stdio: 'inherit',
  });

  const size = statSync(outPath).size;
  // ffmpeg source tarball is typically 12-15 MB. <1 MB means we got
  // a 404 page or an HTML error rendered to disk.
  if (size < 1024 * 1024) {
    console.error(
      `Downloaded tarball is suspiciously small (${size} bytes). Did ffmpeg.org rename or remove this release? URL: ${url}`
    );
    process.exit(69); // EX_UNAVAILABLE
  }

  console.log(`Wrote ${outPath} (${(size / 1024 / 1024).toFixed(1)} MiB).`);
  console.log(
    `\nAttach to the GitHub release alongside binaries:\n  gh release upload vX.Y.Z ${outPath}\n`
  );
}

main();
