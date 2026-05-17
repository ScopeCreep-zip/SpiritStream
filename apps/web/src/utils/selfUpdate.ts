/**
 * Thin wrapper around `@tauri-apps/plugin-updater` + the Rust-side
 * `updater_supported` command. Centralizes the state machine so the
 * Settings About card stays declarative.
 *
 * Trust model: each release artifact is signed locally on the
 * maintainer's machine with `tauri signer sign`, producing a `.sig`
 * file. The plugin verifies that signature against the ed25519
 * pubkey embedded in `tauri.conf.json` `plugins.updater.pubkey`.
 * If verification fails, `downloadAndInstall` throws — the UI
 * surfaces the error and also records an audit-log entry server-side
 * (see `AuditAction::AppUpdateSignatureFailed`).
 *
 * Linux .deb/.rpm installs opt out via `updater_supported()` which
 * returns `false` when the runtime isn't AppImage. The UI hides the
 * Check button in that case — apt/dnf owns updates there.
 */

import { invoke } from '@tauri-apps/api/core';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch as tauriRelaunch } from '@tauri-apps/plugin-process';

/** Per-event download progress callback shape. */
export interface DownloadProgress {
  /** Bytes received so far. */
  downloaded: number;
  /** Total bytes (when known — some upstream servers omit Content-Length). */
  total: number | null;
}

/**
 * Returns `true` when the running install can self-update. Currently:
 *   - macOS, Windows: always `true`.
 *   - Linux AppImage: `true` (detected via the `APPIMAGE` env var per
 *     appimage.org's runtime contract).
 *   - Linux .deb / .rpm: `false` — distro package manager owns updates.
 *
 * Implemented as a Rust-side Tauri command rather than JS-only because
 * `process.env` isn't accessible from the webview in Tauri 2 (the env
 * is read from the Rust process at startup).
 */
export async function isUpdaterSupported(): Promise<boolean> {
  try {
    return await invoke<boolean>('updater_supported');
  } catch {
    // If the command isn't registered (e.g. running in plain browser
    // dev mode without Tauri), surface as "not supported" — the UI
    // hides the Check button. Same outcome as on a distro-packaged
    // Linux build.
    return false;
  }
}

/**
 * Check the configured update endpoint for a newer release. Returns
 * `null` when already up-to-date. Throws on network or signature
 * errors so the caller can surface the failure to the user.
 */
export async function checkForUpdate(): Promise<Update | null> {
  const update = await check();
  if (!update) return null;
  return update;
}

/**
 * Apply an `Update` returned by `checkForUpdate`. Downloads the new
 * artifact, verifies its `.sig` against the embedded pubkey, and
 * unpacks/replaces. `onProgress` is invoked during download.
 */
export async function downloadAndInstall(
  update: Update,
  onProgress?: (p: DownloadProgress) => void,
): Promise<void> {
  let downloaded = 0;
  let total: number | null = null;
  await update.downloadAndInstall((event) => {
    switch (event.event) {
      case 'Started':
        total = event.data.contentLength ?? null;
        downloaded = 0;
        onProgress?.({ downloaded, total });
        break;
      case 'Progress':
        downloaded += event.data.chunkLength;
        onProgress?.({ downloaded, total });
        break;
      case 'Finished':
        onProgress?.({ downloaded, total });
        break;
    }
  });
}

/** Restart the app — required after a successful download/install. */
export async function relaunch(): Promise<void> {
  await tauriRelaunch();
}
