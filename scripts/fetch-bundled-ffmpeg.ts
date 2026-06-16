#!/usr/bin/env tsx
/**
 * Local-dev FFmpeg sidecar fetcher.
 *
 * Tauri declares FFmpeg as a bundled sidecar (`externalBin` in
 * apps/tauri/src-tauri/tauri.conf.json), so the binary file MUST exist at
 * `apps/tauri/src-tauri/binaries/ffmpeg-<TARGET>(.exe)` for `tauri dev`
 * and `tauri build` to run — a system FFmpeg on $PATH does NOT satisfy
 * this. This script makes that file appear so a contributor can
 * `pnpm dev:desktop` without setting up the cross-platform build chain.
 *
 * The per-platform static binaries are built from ffmpeg.org source by
 * `.github/workflows/build-ffmpeg-sidecars.yml` and published to a stable,
 * version-pinned release tag `ffmpeg-sidecar-v<VERSION>`, where VERSION is
 * the `ffmpegVersion` pin in `scripts/ffmpeg-pins.json`. This script reads
 * that pin and downloads the matching binary over plain HTTPS — the
 * repository is public, so no `gh` CLI and no authentication are needed.
 *
 * Resolution order:
 *   1. If the target binary is already present and non-empty → done.
 *   2. Download `ffmpeg-<TARGET>` from the `ffmpeg-sidecar-v<VERSION>`
 *      release over HTTPS.
 *   3. Otherwise → print clear next-step instructions, exit non-zero.
 *
 * The script intentionally does NOT silently fall through. The fallback
 * paths a developer has when the asset is unreachable are documented in
 * the error message:
 *   (a) Retry — the asset may be momentarily unreachable, or the sidecar
 *       release for the current pin has not been built yet (run the
 *       "Build FFmpeg dev sidecars" workflow).
 *   (b) `bash scripts/build-ffmpeg/build.sh <target>` to compile
 *       locally (30+ min, requires platform build deps).
 *   (c) Install ffmpeg via the system package manager — only useful for
 *       the CLI / server paths; the Tauri desktop shell still needs the
 *       sidecar file from (a)/(b).
 *
 * CI does NOT use this script — the publish-tauri matrix consumes the
 * compiled binary directly from the build job's workflow artifact via
 * actions/download-artifact.
 */

import {
  chmodSync,
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

type Target =
  | 'aarch64-apple-darwin'
  | 'x86_64-apple-darwin'
  | 'x86_64-pc-windows-msvc'
  | 'x86_64-unknown-linux-gnu'
  | 'aarch64-unknown-linux-gnu';

const REPO = 'ScopeCreep-zip/SpiritStream';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const projectRoot = join(__dirname, '..');
const binariesDir = join(projectRoot, 'apps', 'tauri', 'src-tauri', 'binaries');
const pinsPath = join(projectRoot, 'scripts', 'ffmpeg-pins.json');

function pinnedFfmpegVersion(): string {
  const pins = JSON.parse(readFileSync(pinsPath, 'utf8')) as { ffmpegVersion?: string };
  if (!pins.ffmpegVersion) {
    throw new Error(`ffmpegVersion missing from ${pinsPath}`);
  }
  return pins.ffmpegVersion;
}

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
    return process.arch === 'arm64' ? 'aarch64-unknown-linux-gnu' : 'x86_64-unknown-linux-gnu';
  }
  throw new Error(`Unsupported platform: ${process.platform}`);
}

function releaseAssetUrl(version: string, target: Target, ext: string): string {
  return `https://github.com/${REPO}/releases/download/ffmpeg-sidecar-v${version}/ffmpeg-${target}${ext}`;
}

function printError(target: Target, ext: string, dest: string, url: string): void {
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
      `\n  - dev-sidecar release asset ${url} (not found or unreachable)` +
      `\n` +
      `\nThree ways to unblock local dev:\n` +
      `\n  (a) Retry — the asset may be momentarily unreachable, or the sidecar` +
      `\n      release for the current ffmpeg pin has not been built yet. Trigger` +
      `\n      the "Build FFmpeg dev sidecars" GitHub Actions workflow, then:` +
      `\n        pnpm tsx scripts/fetch-bundled-ffmpeg.ts --force` +
      `\n` +
      `\n  (b) Compile FFmpeg from source locally (30+ min, requires platform deps):` +
      `\n        bash ${buildScript} ${target}` +
      `\n        cp ffmpeg-bin-${target}/ffmpeg-${target}${ext} ` +
      `apps/tauri/src-tauri/binaries/` +
      `\n` +
      `\n  (c) Install FFmpeg via your system package manager — useful for the CLI /` +
      `\n      server paths only; the Tauri desktop shell still needs the sidecar` +
      `\n      file from (a) or (b):` +
      `\n        ${sysPkgMgr}` +
      `\n`
  );
}

async function downloadSidecarBinary(
  version: string,
  target: Target,
  ext: string,
  dest: string
): Promise<boolean> {
  const url = releaseAssetUrl(version, target, ext);
  console.log(`==> Downloading ffmpeg-${target}${ext} (v${version}) from the dev-sidecar release`);
  console.log(`    ${url}`);

  let res: Response;
  try {
    // The asset URL is a stable public redirect to the asset's storage
    // URL; fetch follows it by default. No auth and no `gh` CLI needed
    // because the repository is public.
    res = await fetch(url, { redirect: 'follow' });
  } catch (e) {
    console.error(`  download failed: ${(e as Error).message}`);
    return false;
  }

  if (res.status === 404) {
    // Release the undici socket so the event loop can drain; leaving an
    // unconsumed body open crashes libuv on Windows at process exit.
    await res.body?.cancel().catch(() => {});
    console.error('  asset not found on the dev-sidecar release (HTTP 404).');
    return false;
  }
  if (!res.ok) {
    await res.body?.cancel().catch(() => {});
    console.error(`  unexpected response: ${res.status} ${res.statusText}`);
    return false;
  }

  const tmp = `${dest}.partial`;
  try {
    const bytes = Buffer.from(await res.arrayBuffer());
    if (bytes.length === 0) {
      console.error('  downloaded asset is empty.');
      return false;
    }
    writeFileSync(tmp, bytes);
    renameSync(tmp, dest);
  } catch (e) {
    rmSync(tmp, { force: true });
    console.error(`  write failed: ${(e as Error).message}`);
    return false;
  }

  // Ensure executable bit on Unix; the release asset is a plain file.
  if (process.platform !== 'win32') {
    chmodSync(dest, 0o755);
  }
  return true;
}

async function main(): Promise<void> {
  const target = detectTarget();
  const version = pinnedFfmpegVersion();
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
          'skipping fetch. Pass --force to refresh from the dev-sidecar release.'
      );
      return;
    }
  }

  // 2. Download the matching binary from the version-pinned sidecar release.
  if (await downloadSidecarBinary(version, target, ext, dest)) {
    const size = statSync(dest).size;
    console.log(
      `==> Installed ${dest} (${(size / 1024 / 1024).toFixed(1)} MiB) from ffmpeg-sidecar-v${version}`
    );
    return;
  }

  // 3. No fallback — fail loud with next-step instructions.
  printError(target, ext, dest, releaseAssetUrl(version, target, ext));
  process.exitCode = 69; // EX_UNAVAILABLE
}

main().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
