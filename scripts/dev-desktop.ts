#!/usr/bin/env tsx
/**
 * `tauri dev` wrapper that negotiates the Vite dev-server port with the
 * OS instead of squatting on the Tauri-conventional 1420 (which every
 * other Tauri project on the machine also wants). The backend API port
 * is negotiated separately by the server itself (it binds port 0 and
 * publishes the result to `run/server.port`).
 *
 * How: ask the OS for a free port, then launch the Tauri CLI with
 *   --config '{"build":{"devUrl":"http://localhost:<P>"}}'
 * (inline JSON merge patches are first-class in tauri-cli 2.x) and
 * SPIRITSTREAM_WEB_PORT=<P> in the environment, which the CLI passes
 * down to the `beforeDevCommand` child where `apps/web/vite.config.ts`
 * reads it. Vite keeps `strictPort: true`, so the tiny window between
 * releasing the probe socket and Vite binding fails loud, not silent.
 *
 * The CLI is spawned WITHOUT a shell (node + the CLI's own tauri.js
 * entry) so the JSON survives Windows cmd.exe quoting.
 */

import { spawn } from 'child_process';
import { createServer } from 'net';
import { createRequire } from 'module';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
const tauriAppDir = join(repoRoot, 'apps', 'tauri');

function findFreePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const probe = createServer();
    probe.once('error', reject);
    probe.listen(0, '127.0.0.1', () => {
      const address = probe.address();
      if (address === null || typeof address === 'string') {
        probe.close(() => reject(new Error('port probe returned no address')));
        return;
      }
      const { port } = address;
      probe.close(() => resolve(port));
    });
  });
}

async function main(): Promise<void> {
  const webPort = await findFreePort();
  const devUrl = `http://localhost:${webPort}`;
  console.log(`[dev-desktop] Vite dev server will use OS-negotiated port ${webPort}`);

  // Resolve the Tauri CLI's JS entry from apps/tauri's own dependency
  // tree (it is a devDependency there, not at the workspace root).
  const require = createRequire(join(tauriAppDir, 'package.json'));
  const tauriJs = require.resolve('@tauri-apps/cli/tauri.js');

  const configPatch = JSON.stringify({ build: { devUrl } });
  const child = spawn(
    process.execPath,
    [tauriJs, 'dev', '--config', configPatch, ...process.argv.slice(2)],
    {
      cwd: tauriAppDir,
      stdio: 'inherit',
      env: { ...process.env, SPIRITSTREAM_WEB_PORT: String(webPort) },
    }
  );

  const forward = (signal: NodeJS.Signals): void => {
    child.kill(signal);
  };
  process.on('SIGINT', forward);
  process.on('SIGTERM', forward);

  child.on('exit', (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
      return;
    }
    process.exit(code ?? 1);
  });
}

main().catch((err) => {
  console.error('[dev-desktop] failed:', err);
  process.exit(1);
});
