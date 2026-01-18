#!/usr/bin/env node
/**
 * Cross-platform wrapper for distribution patching.
 *
 * On Linux (when built in Nix): Patches binary to use FHS paths
 * On macOS/Windows: No-op (not needed)
 */

const { execSync, spawnSync } = require('child_process');
const os = require('os');
const fs = require('fs');
const path = require('path');

const platform = os.platform();

if (platform !== 'linux') {
    console.log(`Skipping binary patching (platform: ${platform})`);
    process.exit(0);
}

// On Linux, delegate to the bash script
const scriptPath = path.join(__dirname, 'patch-for-distribution.sh');

if (!fs.existsSync(scriptPath)) {
    console.log('patch-for-distribution.sh not found, skipping');
    process.exit(0);
}

console.log('Running Linux binary patching...');

const result = spawnSync('bash', [scriptPath], {
    stdio: 'inherit',
    shell: false
});

process.exit(result.status || 0);
