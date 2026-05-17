#!/usr/bin/env tsx
/**
 * Bump the pinned FFmpeg source-tarball version in `scripts/ffmpeg-pins.json`.
 *
 * Under the build-from-source pipeline, this script is the only thing
 * that touches the pin file. It:
 *
 *   1. Downloads `https://ffmpeg.org/releases/ffmpeg-<version>.tar.xz`
 *      and the `.asc` detached signature.
 *   2. Imports the FFmpeg release-signing key from ffmpeg.org's own
 *      HTTPS host and asserts the fingerprint matches the existing
 *      pin (catches a substituted key on the upstream).
 *   3. Verifies the detached signature against the tarball bytes.
 *   4. Computes SHA-256 of the tarball.
 *   5. Rewrites `scripts/ffmpeg-pins.json` with the new version + SHA.
 *
 * No per-platform binary URLs to manage — CI compiles from this one
 * source on every runner.
 *
 * Usage:
 *   pnpm tsx scripts/bump-ffmpeg-pin.ts --to 8.1.2
 *   pnpm tsx scripts/bump-ffmpeg-pin.ts --to 8.1.2 --dry-run
 *
 * Requires `gpg` and `curl` available in PATH on the maintainer's
 * machine. The verification is the same set of checks
 * `scripts/build-ffmpeg/verify-source.sh` does in CI — running both
 * means the maintainer's bump is exercised end-to-end before push.
 */

import { execFileSync, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  createReadStream,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

interface PinFile {
  _?: string;
  ffmpegVersion: string;
  sourceSha256: string;
  gpgFingerprint: string;
}

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const pinsPath = join(__dirname, 'ffmpeg-pins.json');

function getArg(name: string): string | undefined {
  const idx = process.argv.indexOf(name);
  if (idx === -1 || idx === process.argv.length - 1) return undefined;
  return process.argv[idx + 1];
}

function sha256File(path: string): Promise<string> {
  return new Promise((resolve, reject) => {
    const hash = createHash('sha256');
    const stream = createReadStream(path);
    stream.on('error', reject);
    stream.on('data', (chunk) => hash.update(chunk));
    stream.on('end', () => resolve(hash.digest('hex')));
  });
}

function curl(url: string, dest: string): void {
  execFileSync('curl', ['-fLsS', '--retry', '3', '--retry-delay', '2', '-o', dest, url], {
    stdio: 'inherit',
  });
}

/**
 * Run `gpg` with a throwaway home directory so we don't pollute the
 * maintainer's keyring. Returns the fingerprint of the single imported
 * key (uppercased, no separators), and runs the detached-signature
 * verification — throwing on either mismatch.
 */
function verifyGpg(asc: string, tarball: string, expectedFingerprint: string): void {
  const gpgHome = mkdtempSync(join(tmpdir(), 'ffmpeg-bump-gpg-'));
  try {
    const env = { ...process.env, GNUPGHOME: gpgHome };

    const keyPath = join(gpgHome, 'ffmpeg-devel.asc');
    curl('https://ffmpeg.org/ffmpeg-devel.asc', keyPath);
    execFileSync('gpg', ['--batch', '--import', keyPath], { env, stdio: 'inherit' });

    const listed = spawnSync('gpg', ['--list-keys', '--with-colons'], {
      env,
      encoding: 'utf8',
    });
    if (listed.status !== 0) {
      throw new Error(`gpg --list-keys failed: ${listed.stderr}`);
    }
    const fprLine = listed.stdout
      .split(/\r?\n/)
      .find((l) => l.startsWith('fpr:'));
    const importedFingerprint = fprLine ? fprLine.split(':')[9] : '';
    if (!importedFingerprint) {
      throw new Error('gpg returned no key fingerprint after import');
    }
    if (
      importedFingerprint.toUpperCase() !== expectedFingerprint.toUpperCase()
    ) {
      throw new Error(
        `FFmpeg signing key fingerprint mismatch:\n` +
          `  expected: ${expectedFingerprint}\n` +
          `  got:      ${importedFingerprint}`,
      );
    }

    const verify = spawnSync('gpg', ['--batch', '--verify', asc, tarball], {
      env,
      encoding: 'utf8',
    });
    if (verify.status !== 0) {
      throw new Error(
        `gpg --verify rejected the signature:\n${verify.stderr || verify.stdout}`,
      );
    }
    if (!verify.stderr.includes('Good signature')) {
      throw new Error(
        `gpg --verify did not report a Good signature:\n${verify.stderr}`,
      );
    }
  } finally {
    rmSync(gpgHome, { recursive: true, force: true });
  }
}

async function main(): Promise<void> {
  const newVersion = getArg('--to');
  const dryRun = process.argv.includes('--dry-run');

  if (!newVersion) {
    console.error('Usage: bump-ffmpeg-pin.ts --to <version> [--dry-run]');
    process.exit(64); // EX_USAGE
  }

  const raw = readFileSync(pinsPath, 'utf8');
  const pins = JSON.parse(raw) as PinFile;

  const expectedFingerprint = pins.gpgFingerprint;
  if (!expectedFingerprint) {
    console.error('ffmpeg-pins.json missing gpgFingerprint — cannot proceed.');
    process.exit(65);
  }

  const oldVersion = pins.ffmpegVersion;
  console.log(`==> Bumping FFmpeg pin: ${oldVersion} → ${newVersion}`);

  const tmp = mkdtempSync(join(tmpdir(), 'ffmpeg-bump-'));
  try {
    const tarballName = `ffmpeg-${newVersion}.tar.xz`;
    const tarballPath = join(tmp, tarballName);
    const ascPath = `${tarballPath}.asc`;

    console.log(`==> Downloading https://ffmpeg.org/releases/${tarballName}`);
    curl(`https://ffmpeg.org/releases/${tarballName}`, tarballPath);
    console.log(`==> Downloading https://ffmpeg.org/releases/${tarballName}.asc`);
    curl(`https://ffmpeg.org/releases/${tarballName}.asc`, ascPath);

    const size = statSync(tarballPath).size;
    if (size < 1024 * 1024) {
      throw new Error(
        `Tarball too small (${size} bytes) — likely a 404 page from ffmpeg.org. ` +
          `Did the version ${newVersion} get retracted?`,
      );
    }

    console.log('==> Verifying GPG signature');
    verifyGpg(ascPath, tarballPath, expectedFingerprint);

    console.log('==> Computing SHA-256');
    const sha256 = await sha256File(tarballPath);

    pins.ffmpegVersion = newVersion;
    pins.sourceSha256 = sha256;

    const updated = JSON.stringify(pins, null, 2) + '\n';
    if (dryRun) {
      console.log('\n--- DRY RUN — would write:\n');
      console.log(updated);
      return;
    }
    writeFileSync(pinsPath, updated);
    console.log(`\n==> Wrote ${pinsPath}`);
    console.log(`    ffmpegVersion: ${newVersion}`);
    console.log(`    sourceSha256:  ${sha256}`);
    console.log('\nNext steps:');
    console.log('  git diff scripts/ffmpeg-pins.json');
    console.log(`  git commit -am "ffmpeg: bump pin to ${newVersion}"`);
    console.log('  git tag vX.Y.Z && git push --tags');
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

main().catch((err) => {
  console.error(err.message ?? err);
  process.exit(1);
});
