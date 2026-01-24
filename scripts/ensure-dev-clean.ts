#!/usr/bin/env tsx
import { execSync } from 'child_process';
import net from 'net';

type ProcessInfo = {
  pid: number;
  name?: string;
  commandLine?: string;
};

type CliOptions = {
  portsOverride: number[] | null;
  force: boolean;
};

const DEFAULT_BACKEND_PORT = 8008;
const DEFAULT_FRONTEND_PORT = 1420;

function parsePorts(input: string): number[] {
  return input
    .split(/[,\s]+/)
    .map((value) => Number.parseInt(value.trim(), 10))
    .filter((value) => Number.isFinite(value) && value > 0 && value < 65536);
}

function readPort(value: string | undefined): number | null {
  if (!value) {
    return null;
  }
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) && parsed > 0 && parsed < 65536 ? parsed : null;
}

function unique(values: number[]): number[] {
  return Array.from(new Set(values));
}

function safeExec(command: string): string {
  try {
    return execSync(command, {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    }).trim();
  } catch {
    return '';
  }
}

function runPowerShell(script: string): string {
  const escaped = script.replace(/"/g, '\\"');
  return safeExec(`powershell -NoProfile -Command "${escaped}"`);
}

function parsePidLines(output: string): number[] {
  return output
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((value) => Number.parseInt(value, 10))
    .filter((value) => Number.isFinite(value));
}

function getPidsByPortWindows(port: number): number[] {
  const psOutput = runPowerShell(
    `Get-NetTCPConnection -LocalPort ${port} -State Listen -ErrorAction SilentlyContinue | Select-Object -ExpandProperty OwningProcess`,
  );
  const pids = parsePidLines(psOutput);
  if (pids.length > 0) {
    return unique(pids);
  }

  const netstatOutput = safeExec('netstat -ano -p tcp');
  const results: number[] = [];
  for (const line of netstatOutput.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed.startsWith('TCP')) {
      continue;
    }
    const parts = trimmed.split(/\s+/);
    if (parts.length < 5) {
      continue;
    }
    const local = parts[1] ?? '';
    const state = parts[3] ?? '';
    if (state.toUpperCase() !== 'LISTENING') {
      continue;
    }
    if (!local.includes(`:${port}`)) {
      continue;
    }
    const pid = Number.parseInt(parts[4] ?? '', 10);
    if (Number.isFinite(pid)) {
      results.push(pid);
    }
  }

  return unique(results);
}

function getPidsByPortUnix(port: number): number[] {
  const lsofOutput = safeExec(`lsof -n -P -t -iTCP:${port} -sTCP:LISTEN`);
  const lsofPids = parsePidLines(lsofOutput);
  if (lsofPids.length > 0) {
    return unique(lsofPids);
  }

  const ssOutput = safeExec('ss -ltnp');
  const matches: number[] = [];
  for (const line of ssOutput.split(/\r?\n/)) {
    if (!line.includes(`:${port}`)) {
      continue;
    }
    const regex = /pid=(\d+)/g;
    let match = regex.exec(line);
    while (match) {
      const pid = Number.parseInt(match[1] ?? '', 10);
      if (Number.isFinite(pid)) {
        matches.push(pid);
      }
      match = regex.exec(line);
    }
  }

  return unique(matches);
}

function getPidsByPort(port: number): number[] {
  if (process.platform === 'win32') {
    return getPidsByPortWindows(port);
  }
  return getPidsByPortUnix(port);
}

function getProcessInfoWindows(pid: number): ProcessInfo {
  const output = runPowerShell(
    `Get-CimInstance Win32_Process -Filter 'ProcessId=${pid}' | Select-Object ProcessId,Name,CommandLine | ConvertTo-Json -Compress`,
  );
  if (!output) {
    return { pid };
  }

  try {
    const parsed = JSON.parse(output) as { Name?: string; CommandLine?: string } | Array<{ Name?: string; CommandLine?: string }>;
    const record = Array.isArray(parsed) ? parsed[0] : parsed;
    return {
      pid,
      name: record?.Name,
      commandLine: record?.CommandLine,
    };
  } catch {
    return { pid };
  }
}

function getProcessInfoUnix(pid: number): ProcessInfo {
  const name = safeExec(`ps -p ${pid} -o comm=`).trim();
  const commandLine = safeExec(`ps -p ${pid} -o args=`).trim();
  return {
    pid,
    name: name || undefined,
    commandLine: commandLine || undefined,
  };
}

function getProcessInfo(pid: number): ProcessInfo {
  if (process.platform === 'win32') {
    return getProcessInfoWindows(pid);
  }
  return getProcessInfoUnix(pid);
}

function killPid(pid: number): void {
  if (process.platform === 'win32') {
    safeExec(`taskkill /PID ${pid} /T /F`);
    return;
  }

  try {
    process.kill(pid, 'SIGTERM');
  } catch {
    return;
  }
}

function isPortAvailable(port: number): Promise<boolean> {
  return new Promise((resolve) => {
    const server = net.createServer();
    server.once('error', () => resolve(false));
    server.once('listening', () => {
      server.close(() => resolve(true));
    });
    server.listen(port, '127.0.0.1');
  });
}

async function waitForPortAvailable(port: number, attempts = 8, delayMs = 250): Promise<boolean> {
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    if (await isPortAvailable(port)) {
      return true;
    }
    await new Promise((resolve) => setTimeout(resolve, delayMs));
  }
  return false;
}

function parseArgs(args: string[]): CliOptions {
  let portsOverride: number[] | null = null;
  let force = false;

  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === '--force') {
      force = true;
      continue;
    }
    if (arg === '--ports' || arg === '--port') {
      const value = args[i + 1];
      if (value) {
        portsOverride = parsePorts(value);
        i += 1;
      }
      continue;
    }
    if (arg.startsWith('--ports=') || arg.startsWith('--port=')) {
      const value = arg.split('=')[1] ?? '';
      portsOverride = parsePorts(value);
      continue;
    }
    if (arg === '--help' || arg === '-h') {
      console.log('Usage: tsx scripts/ensure-dev-clean.ts [--ports 8008,1420] [--force]');
      process.exit(0);
    }
  }

  return { portsOverride, force };
}

function tokensForPort(port: number, backendPort: number): string[] {
  if (port === backendPort || port === DEFAULT_BACKEND_PORT) {
    return ['spiritstream-server', 'spiritstream'];
  }
  if (port === DEFAULT_FRONTEND_PORT) {
    return ['vite', 'tauri', 'spiritstream'];
  }
  return ['spiritstream', 'spiritstream-server', 'vite', 'tauri', 'pnpm', 'tsx'];
}

async function main(): Promise<void> {
  const options = parseArgs(process.argv.slice(2));
  const backendPort = readPort(process.env['SPIRITSTREAM_PORT']) ?? DEFAULT_BACKEND_PORT;

  const defaultPorts = unique([backendPort, DEFAULT_FRONTEND_PORT]);
  const ports = options.portsOverride && options.portsOverride.length > 0
    ? unique(options.portsOverride)
    : defaultPorts;

  let blocked = false;

  for (const port of ports) {
    const pids = getPidsByPort(port);
    if (pids.length === 0) {
      console.log(`[dev-clean] Port ${port} is free.`);
      continue;
    }

    for (const pid of pids) {
      const info = getProcessInfo(pid);
      const tokens = tokensForPort(port, backendPort);
      const haystack = `${info.name ?? ''} ${info.commandLine ?? ''}`.toLowerCase();
      const matched = tokens.some((token) => haystack.includes(token));

      if (!matched && !options.force) {
        console.warn(
          `[dev-clean] Port ${port} is in use by pid ${pid}. Unable to verify it is a SpiritStream dev process. Use --force to terminate.`,
        );
        blocked = true;
        continue;
      }

      console.log(
        `[dev-clean] Stopping pid ${pid} on port ${port}${info.commandLine ? ` (${info.commandLine})` : ''}.`,
      );
      killPid(pid);
    }

    const released = await waitForPortAvailable(port);
    if (!released) {
      console.warn(`[dev-clean] Port ${port} is still in use after cleanup attempts.`);
      blocked = true;
    }
  }

  if (blocked) {
    process.exit(1);
  }
}

main().catch((error) => {
  console.error(`[dev-clean] Failed to ensure clean dev ports: ${String(error)}`);
  process.exit(1);
});
