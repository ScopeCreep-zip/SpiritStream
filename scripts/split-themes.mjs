#!/usr/bin/env node

/**
 * Split dual-mode themes into separate light and dark theme files
 * This migrates from the old format (tokens.light + tokens.dark) to the new format (single mode per file)
 */

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const themesDir = path.join(__dirname, '..', 'themes');

function parseJSONC(content) {
  // Remove comments from JSONC
  const withoutComments = content.replace(/\/\/.*$/gm, '').replace(/\/\*[\s\S]*?\*\//g, '');
  return JSON.parse(withoutComments);
}

function splitTheme(filename) {
  const filePath = path.join(themesDir, filename);

  if (!fs.existsSync(filePath)) {
    console.error(`❌ Theme file not found: ${filename}`);
    return;
  }

  console.log(`\n📦 Processing: ${filename}`);

  const content = fs.readFileSync(filePath, 'utf-8');
  const theme = parseJSONC(content);

  // Check if it's a dual-mode theme
  if (!theme.tokens || !theme.tokens.light || !theme.tokens.dark) {
    console.log(`⚠️  Skipping ${filename} - not a dual-mode theme`);
    return;
  }

  const baseName = theme.id || path.basename(filename, '.jsonc');

  // Create light variant
  const lightTheme = {
    id: `${baseName}-light`,
    name: `${theme.name} Light`,
    mode: 'light',
    tokens: theme.tokens.light,
  };

  // Create dark variant
  const darkTheme = {
    id: `${baseName}-dark`,
    name: `${theme.name} Dark`,
    mode: 'dark',
    tokens: theme.tokens.dark,
  };

  // Write light theme
  const lightPath = path.join(themesDir, `${baseName}-light.jsonc`);
  fs.writeFileSync(
    lightPath,
    JSON.stringify(lightTheme, null, 2),
    'utf-8'
  );
  console.log(`✅ Created: ${baseName}-light.jsonc`);

  // Write dark theme
  const darkPath = path.join(themesDir, `${baseName}-dark.jsonc`);
  fs.writeFileSync(
    darkPath,
    JSON.stringify(darkTheme, null, 2),
    'utf-8'
  );
  console.log(`✅ Created: ${baseName}-dark.jsonc`);

  // Rename original to .deprecated
  const deprecatedPath = path.join(themesDir, `${filename}.deprecated`);
  fs.renameSync(filePath, deprecatedPath);
  console.log(`📋 Renamed ${filename} to ${filename}.deprecated`);
}

// Main execution
console.log('🎨 Theme Splitter - Converting dual-mode themes to single-mode\n');
console.log('This will split themes with both light and dark modes into separate files.\n');

const themesToSplit = [
  'spirit.jsonc',
  'dracula.jsonc',
  'nord.jsonc',
  'catppuccin-mocha.jsonc',
  'rainbow-pride.jsonc',
  'trans-pride.jsonc',
];

for (const theme of themesToSplit) {
  splitTheme(theme);
}

console.log('\n✨ Theme splitting complete!');
console.log('\nNext steps:');
console.log('1. Review the new theme files in the themes/ directory');
console.log('2. Test that the app loads the new themes correctly');
console.log('3. Update the frontend to use the new single-mode format');
console.log('4. The .deprecated files can be deleted once migration is confirmed\n');
