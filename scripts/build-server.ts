#!/usr/bin/env tsx
/**
 * Build the server sidecar binary for Tauri bundling.
 *
 * Tauri expects sidecar binaries in src-tauri/binaries/ with platform-specific names:
 * - Windows: server-x86_64-pc-windows-msvc.exe
 * - Linux: server-x86_64-unknown-linux-gnu
 * - macOS Intel: server-x86_64-apple-darwin
 * - macOS ARM: server-aarch64-apple-darwin
 */

import { execSync } from 'child_process';
import { copyFileSync, mkdirSync, existsSync, writeFileSync } from 'fs';
import { join } from 'path';

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

// Create binaries directory
const binariesDir = join('src-tauri', 'binaries');
if (!existsSync(binariesDir)) {
  mkdirSync(binariesDir, { recursive: true });
}

// Destination path for the sidecar binary
const destPath = join(binariesDir, `server-${target}${ext}`);

// Create placeholder if it doesn't exist (so Tauri build.rs doesn't fail during library compilation)
if (!existsSync(destPath)) {
  console.log('Creating placeholder for Tauri build validation...');
  writeFileSync(destPath, '');
}

// Build the server binary
const buildCmd = isRelease
  ? `cargo build --manifest-path src-tauri/Cargo.toml --bin server --release`
  : `cargo build --manifest-path src-tauri/Cargo.toml --bin server`;

try {
  execSync(buildCmd, { stdio: 'inherit' });
} catch (error) {
  console.error('Failed to build server binary');
  process.exit(1);
}

// Copy binary with platform-specific name
const sourcePath = join('src-tauri', 'target', profile, `server${ext}`);

console.log(`Copying ${sourcePath} to ${destPath}...`);
copyFileSync(sourcePath, destPath);

console.log('Server binary built successfully!');
