#!/usr/bin/env tsx
import { spawnSync } from 'child_process';
import { fileURLToPath } from 'url';

function parseFeatures(args: string[]): { features: string | null; remaining: string[] } {
  let features = '';
  const remaining: string[] = [];

  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === '--features' && args[i + 1]) {
      features = [features, args[i + 1]].filter(Boolean).join(',');
      i += 1;
      continue;
    }
    if (arg.startsWith('--features=')) {
      const value = arg.split('=')[1] || '';
      features = [features, value].filter(Boolean).join(',');
      continue;
    }
    remaining.push(arg);
  }

  const envFeatures = process.env['SPIRITSTREAM_SERVER_FEATURES']
    || process.env['npm_config_features']
    || '';
  const merged = [features, envFeatures]
    .filter(Boolean)
    .join(',')
    .split(/[,\s]+/)
    .map((feature) => feature.trim())
    .filter(Boolean)
    .join(',');

  return { features: merged.length > 0 ? merged : null, remaining };
}

function runCommand(command: string, args: string[], env: NodeJS.ProcessEnv): void {
  const result = spawnSync(command, args, {
    stdio: 'inherit',
    env,
    shell: process.platform === 'win32',
  });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

const { features, remaining } = parseFeatures(process.argv.slice(2));
const env = { ...process.env };
if (features) {
  env.SPIRITSTREAM_SERVER_FEATURES = features;
}

const ensureCleanPath = fileURLToPath(new URL('./ensure-dev-clean.ts', import.meta.url));

runCommand('pnpm', ['exec', 'tsx', ensureCleanPath], env);
runCommand('pnpm', ['run', 'build:server'], env);
runCommand('pnpm', ['exec', 'tauri', 'dev', ...remaining], env);
