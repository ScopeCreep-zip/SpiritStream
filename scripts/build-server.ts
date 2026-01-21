#!/usr/bin/env tsx
/**
 * Build the server sidecar binary for Tauri bundling.
 *
 * This script builds the standalone server from /server/ and copies it to
 * apps/desktop/src-tauri/binaries/ with platform-specific names:
 * - Windows: spiritstream-server-x86_64-pc-windows-msvc.exe
 * - Linux: spiritstream-server-x86_64-unknown-linux-gnu
 * - macOS Intel: spiritstream-server-x86_64-apple-darwin
 * - macOS ARM: spiritstream-server-aarch64-apple-darwin
 */

import { execSync } from 'child_process';
import { copyFileSync, mkdirSync, existsSync, writeFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

// Get project root (scripts/ is one level down from root)
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const projectRoot = join(__dirname, '..');

const isRelease = process.argv.includes('--release');
const profile = isRelease ? 'release' : 'debug';

// Detect platform and architecture
const platform = process.platform;
const arch = process.arch;

// Map to Rust target triple
function getRustTarget(): string {
  if (platform === 'win32') {
    return 'x86_64-pc-windows-msvc';
  } else if (platform === 'darwin') {
    return arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin';
  } else if (platform === 'linux') {
    return arch === 'arm64' ? 'aarch64-unknown-linux-gnu' : 'x86_64-unknown-linux-gnu';
  }
  throw new Error(`Unsupported platform: ${platform}`);
}

const target = getRustTarget();
const ext = platform === 'win32' ? '.exe' : '';

console.log(`Building server binary for ${target} (${profile})...`);

// Create binaries directory in desktop app (using absolute paths)
const binariesDir = join(projectRoot, 'apps', 'desktop', 'src-tauri', 'binaries');
if (!existsSync(binariesDir)) {
  mkdirSync(binariesDir, { recursive: true });
}

// Destination path for the sidecar binary
const destPath = join(binariesDir, `spiritstream-server-${target}${ext}`);

// Create placeholder if it doesn't exist (so Tauri build.rs doesn't fail during library compilation)
if (!existsSync(destPath)) {
  console.log('Creating placeholder for Tauri build validation...');
  writeFileSync(destPath, '');
}

// Build the server binary from /server/ (using absolute path to manifest)
const manifestPath = join(projectRoot, 'server', 'Cargo.toml');
const buildCmd = isRelease
  ? `cargo build --manifest-path "${manifestPath}" --release`
  : `cargo build --manifest-path "${manifestPath}"`;

try {
  execSync(buildCmd, { stdio: 'inherit', cwd: projectRoot });
} catch (error) {
  console.error('Failed to build server binary');
  process.exit(1);
}

// Copy binary with platform-specific name (using absolute paths)
const sourcePath = join(projectRoot, 'server', 'target', profile, `spiritstream-server${ext}`);

console.log(`Copying ${sourcePath} to ${destPath}...`);
copyFileSync(sourcePath, destPath);

console.log('Server binary built successfully!');
