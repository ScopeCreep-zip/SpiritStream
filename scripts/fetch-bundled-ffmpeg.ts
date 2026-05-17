#!/usr/bin/env tsx
/**
 * Local-dev FFmpeg sidecar fetcher.
 *
 * Under the build-from-source pipeline, CI compiles FFmpeg from
 * ffmpeg.org's source tarball on every release-tag push. This script
 * is the local-dev counterpart: it makes the sidecar appear at
 * `apps/tauri/src-tauri/binaries/ffmpeg-<TARGET>(.exe)` so a
 * contributor can `pnpm dev:desktop` without setting up the full
 * cross-platform build chain on their laptop.
 *
 * Resolution order:
 *   1. If the target binary is already present and non-empty → done.
 *   2. If `gh` CLI is installed AND authenticated → download the
 *      latest published release's `ffmpeg-<TARGET>` artifact from
 *      `github.com/ScopeCreep-zip/SpiritStream/releases/latest`.
 *   3. Otherwise → print clear next-step instructions, exit non-zero.
 *
 * The script intentionally does NOT silently fall through. The three
 * fallback paths a developer has are documented in the error message:
 *   (a) `gh auth login` and re-run this script.
 *   (b) `bash scripts/build-ffmpeg/build.sh <target>` to compile
 *       locally (30+ min, requires platform build deps).
 *   (c) Install ffmpeg via the system package manager
 *       (`brew install ffmpeg` / `apt install ffmpeg`); the running
 *       app discovers it via `$PATH` (FFmpegLocator step 3) without
 *       any sidecar — fine for dev.
 *
 * CI does NOT use this script — it goes through the build-ffmpeg
 * job directly and consumes the resulting artifact via
 * actions/download-artifact.
 */

import { execFileSync, spawnSync } from 'node:child_process';
import { chmodSync, existsSync, mkdirSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

type Target =
  | 'aarch64-apple-darwin'
  | 'x86_64-apple-darwin'
  | 'x86_64-pc-windows-msvc'
  | 'x86_64-unknown-linux-gnu'
  | 'aarch64-unknown-linux-gnu';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const projectRoot = join(__dirname, '..');
const binariesDir = join(projectRoot, 'apps', 'tauri', 'src-tauri', 'binaries');

function detectTarget(): Target {
  const explicitIdx = process.argv.indexOf('--target');
  if (explicitIdx !== -1 && process.argv[explicitIdx + 1]) {
    return process.argv[explicitIdx + 1] as Target;
  }
  if (process.platform === 'win32') return 'x86_64-pc-windows-msvc';
  if (process.platform === 'darwin') {
    return process.arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin';
  }
  if (process.platform === 'linux') {
    return process.arch === 'arm64'
      ? 'aarch64-unknown-linux-gnu'
      : 'x86_64-unknown-linux-gnu';
  }
  throw new Error(`Unsupported platform: ${process.platform}`);
}

function ghAvailable(): boolean {
  const r = spawnSync('gh', ['--version'], { stdio: 'ignore' });
  return r.status === 0;
}

function ghAuthenticated(): boolean {
  const r = spawnSync('gh', ['auth', 'status'], { stdio: 'ignore' });
  return r.status === 0;
}

function printErrorAndExit(target: Target, ext: string, dest: string): never {
  const buildScript = 'scripts/build-ffmpeg/build.sh';
  const sysPkgMgr =
    process.platform === 'darwin'
      ? 'brew install ffmpeg'
      : process.platform === 'linux'
        ? 'sudo apt install ffmpeg   # or `sudo dnf install ffmpeg` / `sudo pacman -S ffmpeg`'
        : 'winget install Gyan.FFmpeg';

  console.error(
    `\nfetch-bundled-ffmpeg: no FFmpeg sidecar available for target ${target}.\n` +
      `\nTried:` +
      `\n  - existing binary at ${dest} (not present)` +
      `\n  - latest GitHub Release artifact (gh CLI missing or unauthenticated)` +
      `\n` +
      `\nThree ways to unblock local dev:\n` +
      `\n  (a) Authenticate gh CLI and re-run:` +
      `\n        gh auth login` +
      `\n        pnpm tsx scripts/fetch-bundled-ffmpeg.ts` +
      `\n` +
      `\n  (b) Compile FFmpeg from source locally (30+ min, requires platform deps):` +
      `\n        bash ${buildScript} ${target}` +
      `\n        cp ffmpeg-bin-${target}/ffmpeg-${target}${ext} ` +
      `apps/tauri/src-tauri/binaries/` +
      `\n` +
      `\n  (c) Install FFmpeg via your system package manager — the app will` +
      `\n      discover it via $PATH (FFmpegLocator step 3), no sidecar needed for dev:` +
      `\n        ${sysPkgMgr}` +
      `\n`,
  );
  process.exit(69); // EX_UNAVAILABLE
}

function downloadFromLatestRelease(target: Target, ext: string, dest: string): boolean {
  if (!ghAvailable()) {
    console.log('  gh CLI not installed — skipping release download.');
    return false;
  }
  if (!ghAuthenticated()) {
    console.log('  gh CLI not authenticated (run `gh auth login`) — skipping release download.');
    return false;
  }

  const artifactName = `ffmpeg-${target}${ext}`;
  console.log(`==> Downloading ${artifactName} from latest GitHub Release`);
  try {
    // `gh release download` with --pattern matches the asset name as
    // uploaded by the build-ffmpeg job in release.yml. We pull from
    // `latest` so this stays current as releases ship — for a specific
    // older release, the dev runs the build.sh path instead.
    execFileSync(
      'gh',
      [
        'release',
        'download',
        '--repo',
        'ScopeCreep-zip/SpiritStream',
        '--pattern',
        artifactName,
        '--dir',
        binariesDir,
        '--clobber',
      ],
      { stdio: 'inherit' },
    );
  } catch (e) {
    console.error(`  gh release download failed: ${(e as Error).message}`);
    return false;
  }

  if (!existsSync(dest) || statSync(dest).size === 0) {
    console.error(`  Downloaded artifact missing or empty: ${dest}`);
    return false;
  }
  // Ensure executable bit on Unix; download-artifact does not preserve it.
  if (process.platform !== 'win32') {
    chmodSync(dest, 0o755);
  }
  return true;
}

function main(): void {
  const target = detectTarget();
  const force = process.argv.includes('--force');
  mkdirSync(binariesDir, { recursive: true });
  const ext = target.includes('windows') ? '.exe' : '';
  const dest = join(binariesDir, `ffmpeg-${target}${ext}`);

  // 1. Cached binary present?
  if (!force && existsSync(dest)) {
    const size = statSync(dest).size;
    if (size > 0) {
      console.log(
        `FFmpeg sidecar already present at ${dest} (${(size / 1024 / 1024).toFixed(1)} MiB); ` +
          'skipping fetch. Pass --force to refresh from the latest GitHub Release.',
      );
      return;
    }
  }

  // 2. Try latest GitHub Release.
  if (downloadFromLatestRelease(target, ext, dest)) {
    const size = statSync(dest).size;
    console.log(
      `==> Installed ${dest} (${(size / 1024 / 1024).toFixed(1)} MiB) from latest release`,
    );
    return;
  }

  // 3. No fallback — fail loud with next-step instructions.
  printErrorAndExit(target, ext, dest);
}

main();
