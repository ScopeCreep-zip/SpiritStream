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
import { copyFileSync, mkdirSync, existsSync, writeFileSync, unlinkSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { tmpdir } from 'os';

/**
 * Find vcvarsall.bat using vswhere.exe (the standard Visual Studio locator).
 * This is the same approach used by ilammy/msvc-dev-cmd and other build tools.
 */
function findVcvarsall(): string | null {
  const vswherePaths = [
    join(process.env['ProgramFiles(x86)'] || 'C:\\Program Files (x86)', 'Microsoft Visual Studio', 'Installer', 'vswhere.exe'),
    join(process.env['ProgramFiles'] || 'C:\\Program Files', 'Microsoft Visual Studio', 'Installer', 'vswhere.exe'),
  ];

  for (const vswherePath of vswherePaths) {
    if (existsSync(vswherePath)) {
      try {
        // Query vswhere for VS installation path with VC tools
        const result = execSync(
          `"${vswherePath}" -latest -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`,
          { encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] }
        ).trim();

        if (result) {
          const vcvarsPath = join(result, 'VC', 'Auxiliary', 'Build', 'vcvarsall.bat');
          if (existsSync(vcvarsPath)) {
            return vcvarsPath;
          }
        }
      } catch {
        // vswhere failed, try next path
      }
    }
  }

  return null;
}

/**
 * Check if running in MSYS2/Git Bash environment.
 * These environments can interfere with Rust's MSVC toolchain detection
 * because their COM API emulation doesn't work correctly with Visual Studio's
 * SetupConfiguration interface.
 */
function isRunningInMsys2(): boolean {
  // MSYSTEM is set by MSYS2/Git Bash (e.g., "MINGW64", "MSYS")
  if (process.env.MSYSTEM) {
    return true;
  }
  // Check if PATH starts with Unix-style paths (MSYS2 signature)
  const path = process.env.PATH || '';
  if (path.startsWith('/') || path.includes('/mingw64/') || path.includes('/usr/bin')) {
    return true;
  }
  return false;
}

/**
 * Run cargo command with proper MSVC environment setup.
 *
 * On Windows in MSYS2/Git Bash environments, the COM API detection that
 * Rust uses to find MSVC tools can fail, causing it to fall back to PATH
 * where Git's link.exe shadows MSVC's link.exe.
 *
 * The standard solution (used by ilammy/msvc-dev-cmd, Azure DevOps, etc.)
 * is to run vcvarsall.bat first to set up the environment variables that
 * cc-rs/rustc will use to find the MSVC toolchain.
 *
 * We use a temporary batch file to avoid MSYS2's quote handling issues.
 */
function runCargoCommand(cmd: string, cwd: string): void {
  if (process.platform === 'win32' && isRunningInMsys2()) {
    const vcvarsall = findVcvarsall();

    if (vcvarsall) {
      console.log('Detected MSYS2/Git Bash environment, using vcvarsall.bat for MSVC setup');

      // Create a temporary batch file to run vcvarsall and cargo
      // This avoids all the shell quoting issues
      const batchFile = join(tmpdir(), `spiritstream-build-${Date.now()}.bat`);
      const batchContent = `@echo off
call "${vcvarsall}" x64
if errorlevel 1 exit /b 1
cd /d "${cwd}"
${cmd}
`;
      writeFileSync(batchFile, batchContent);

      try {
        // Run the batch file via cmd
        execSync(`cmd /c "${batchFile}"`, { stdio: 'inherit' });
      } finally {
        // Clean up the temporary batch file
        try {
          unlinkSync(batchFile);
        } catch {
          // Ignore cleanup errors
        }
      }
    } else {
      console.warn(
        'Warning: Running in MSYS2/Git Bash but vcvarsall.bat not found.',
        'MSVC detection may fail. Install Visual Studio Build Tools or run from PowerShell/CMD.'
      );
      execSync(cmd, { stdio: 'inherit', cwd });
    }
  } else {
    // Non-Windows or native Windows shell - run directly
    execSync(cmd, { stdio: 'inherit', cwd });
  }
}

// Get project root (scripts/ is one level down from root)
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const projectRoot = join(__dirname, '..');

const isRelease = process.argv.includes('--release');
const profile = isRelease ? 'release' : 'debug';

// Check for explicit --target argument (for cross-compilation)
function getExplicitTarget(): string | null {
  const targetIndex = process.argv.indexOf('--target');
  if (targetIndex !== -1 && process.argv[targetIndex + 1]) {
    return process.argv[targetIndex + 1];
  }
  return null;
}

function getFeatures(): string | null {
  const fromEnv = process.env['SPIRITSTREAM_SERVER_FEATURES']
    || process.env['npm_config_features']
    || '';
  const args = process.argv;

  let fromArgs = '';
  const featuresIndex = args.indexOf('--features');
  if (featuresIndex !== -1 && args[featuresIndex + 1]) {
    fromArgs = args[featuresIndex + 1];
  } else {
    const inline = args.find((arg) => arg.startsWith('--features='));
    if (inline) {
      fromArgs = inline.split('=')[1] || '';
    }
  }

  const raw = [fromArgs, fromEnv].filter(Boolean).join(',');
  const normalized = raw
    .split(/[,\s]+/)
    .map((feature) => feature.trim())
    .filter(Boolean)
    .join(',');

  return normalized.length > 0 ? normalized : null;
}

// Detect platform and architecture
const platform = process.platform;
const arch = process.arch;

// Map to Rust target triple
function getRustTarget(): string {
  // Use explicit target if provided (for cross-compilation)
  const explicit = getExplicitTarget();
  if (explicit) {
    return explicit;
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

const target = getRustTarget();
const explicitTarget = getExplicitTarget();
const ext = platform === 'win32' ? '.exe' : '';
const features = getFeatures();

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
const targetFlag = explicitTarget ? `--target ${explicitTarget}` : '';
const featuresFlag = features ? `--features ${features}` : '';
const buildCmd = isRelease
  ? `cargo build --manifest-path "${manifestPath}" --release ${targetFlag} ${featuresFlag}`.trim()
  : `cargo build --manifest-path "${manifestPath}" ${targetFlag} ${featuresFlag}`.trim();

try {
  runCargoCommand(buildCmd, projectRoot);
} catch (error) {
  console.error('Failed to build server binary');
  process.exit(1);
}

// Copy binary with platform-specific name (using absolute paths)
// When cross-compiling with --target, binary is in target/{target}/{profile}/
const sourcePath = explicitTarget
  ? join(projectRoot, 'server', 'target', explicitTarget, profile, `spiritstream-server${ext}`)
  : join(projectRoot, 'server', 'target', profile, `spiritstream-server${ext}`);

console.log(`Copying ${sourcePath} to ${destPath}...`);
copyFileSync(sourcePath, destPath);

console.log('Server binary built successfully!');
