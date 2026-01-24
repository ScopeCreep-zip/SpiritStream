#!/usr/bin/env tsx
/**
 * Download go2rtc binary for Tauri bundling.
 *
 * go2rtc is a WebRTC/RTSP server used for low-latency video preview.
 * This script downloads pre-built binaries from GitHub releases and copies
 * them to apps/desktop/src-tauri/binaries/ with platform-specific names:
 * - Windows: go2rtc-x86_64-pc-windows-msvc.exe
 * - Linux: go2rtc-x86_64-unknown-linux-gnu
 * - macOS Intel: go2rtc-x86_64-apple-darwin
 * - macOS ARM: go2rtc-aarch64-apple-darwin
 *
 * @see https://github.com/AlexxIT/go2rtc
 */

import { createWriteStream, existsSync, mkdirSync, chmodSync, unlinkSync, renameSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { pipeline } from 'stream/promises';
import { createHash } from 'crypto';
import { createReadStream } from 'fs';

// go2rtc version to download
const GO2RTC_VERSION = '1.9.14';

// GitHub release asset names for each platform
const PLATFORM_ASSETS: Record<string, string> = {
  'x86_64-pc-windows-msvc': `go2rtc_win64.zip`,
  'x86_64-unknown-linux-gnu': `go2rtc_linux_amd64`,
  'aarch64-unknown-linux-gnu': `go2rtc_linux_arm64`,
  'x86_64-apple-darwin': `go2rtc_mac_amd64.zip`,
  'aarch64-apple-darwin': `go2rtc_mac_arm64.zip`,
};

// Get project root
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const projectRoot = join(__dirname, '..');

// Detect platform and architecture
const platform = process.platform;
const arch = process.arch;

function getRustTarget(): string {
  // Check for explicit --target argument
  const targetIndex = process.argv.indexOf('--target');
  if (targetIndex !== -1 && process.argv[targetIndex + 1]) {
    return process.argv[targetIndex + 1];
  }

  if (platform === 'win32') {
    return 'x86_64-pc-windows-msvc';
  } else if (platform === 'darwin') {
    return arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin';
  } else if (platform === 'linux') {
    return arch === 'arm64' ? 'aarch64-unknown-linux-gnu' : 'x86_64-unknown-linux-gnu';
  }
  throw new Error(`Unsupported platform: ${platform}`);
}

async function downloadFile(url: string, destPath: string): Promise<void> {
  const response = await fetch(url, {
    headers: {
      'User-Agent': 'SpiritStream-Build/1.0',
    },
    redirect: 'follow',
  });

  if (!response.ok) {
    throw new Error(`Failed to download ${url}: ${response.status} ${response.statusText}`);
  }

  const body = response.body;
  if (!body) {
    throw new Error('Response body is null');
  }

  const fileStream = createWriteStream(destPath);
  // @ts-expect-error - Node.js Readable from web ReadableStream
  await pipeline(body, fileStream);
}

async function extractZip(zipPath: string, extractDir: string): Promise<string> {
  const { execSync } = await import('child_process');

  mkdirSync(extractDir, { recursive: true });

  if (platform === 'win32') {
    // Use PowerShell to extract on Windows
    execSync(`powershell -Command "Expand-Archive -Path '${zipPath}' -DestinationPath '${extractDir}' -Force"`, {
      stdio: 'inherit',
    });
  } else {
    // Use unzip on Unix
    execSync(`unzip -o "${zipPath}" -d "${extractDir}"`, {
      stdio: 'inherit',
    });
  }

  // Find the extracted binary
  const { readdirSync } = await import('fs');
  const files = readdirSync(extractDir);
  const binary = files.find(f => f.startsWith('go2rtc'));

  if (!binary) {
    throw new Error('Could not find go2rtc binary in extracted archive');
  }

  return join(extractDir, binary);
}

async function main(): Promise<void> {
  const target = getRustTarget();
  const asset = PLATFORM_ASSETS[target];

  if (!asset) {
    throw new Error(`Unsupported target: ${target}. Supported targets: ${Object.keys(PLATFORM_ASSETS).join(', ')}`);
  }

  const ext = platform === 'win32' ? '.exe' : '';
  const binariesDir = join(projectRoot, 'apps', 'desktop', 'src-tauri', 'binaries');
  const destPath = join(binariesDir, `go2rtc-${target}${ext}`);

  // Check if already downloaded
  if (existsSync(destPath) && !process.argv.includes('--force')) {
    console.log(`go2rtc binary already exists at ${destPath}`);
    console.log('Use --force to re-download');
    return;
  }

  // Create binaries directory
  mkdirSync(binariesDir, { recursive: true });

  const downloadUrl = `https://github.com/AlexxIT/go2rtc/releases/download/v${GO2RTC_VERSION}/${asset}`;
  console.log(`Downloading go2rtc v${GO2RTC_VERSION} for ${target}...`);
  console.log(`URL: ${downloadUrl}`);

  const isZip = asset.endsWith('.zip');
  const tempPath = join(binariesDir, isZip ? `go2rtc-temp.zip` : `go2rtc-temp${ext}`);
  const extractDir = join(binariesDir, 'go2rtc-extract');

  try {
    // Download the file
    await downloadFile(downloadUrl, tempPath);
    console.log('Download complete!');

    if (isZip) {
      // Extract zip file (Windows)
      console.log('Extracting...');
      const extractedBinary = await extractZip(tempPath, extractDir);
      renameSync(extractedBinary, destPath);

      // Cleanup
      unlinkSync(tempPath);
      const { rmSync } = await import('fs');
      rmSync(extractDir, { recursive: true, force: true });
    } else {
      // Direct binary (Linux/macOS)
      renameSync(tempPath, destPath);
    }

    // Make executable on Unix
    if (platform !== 'win32') {
      chmodSync(destPath, 0o755);
    }

    console.log(`go2rtc binary installed at ${destPath}`);

    // Verify the binary works
    const { execSync } = await import('child_process');
    try {
      const version = execSync(`"${destPath}" --version`, { encoding: 'utf8', timeout: 5000 });
      console.log(`Verified: ${version.trim()}`);
    } catch (e) {
      console.warn('Warning: Could not verify binary version (may be quarantined on macOS)');
    }

  } catch (error) {
    // Cleanup on error
    try {
      if (existsSync(tempPath)) unlinkSync(tempPath);
      if (existsSync(extractDir)) {
        const { rmSync } = await import('fs');
        rmSync(extractDir, { recursive: true, force: true });
      }
    } catch {
      // Ignore cleanup errors
    }
    throw error;
  }
}

main().catch((error) => {
  console.error('Failed to download go2rtc:', error);
  process.exit(1);
});
