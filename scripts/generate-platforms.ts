#!/usr/bin/env tsx
/**
 * Generate TypeScript types from streaming-platforms.json
 * Filters to only include RTMP/RTMPS platforms with "append" placement
 */

import * as fs from 'fs';
import * as path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

interface Service {
  name: string;
  displayName?: string;
  defaultUrl: string;
  streamKeyPlacement: 'append' | 'in_url_template';
  abbreviation: string;
  color: string;
  faviconPath?: string;
}

interface PlatformsJSON {
  services: Service[];
}

// Calculate appropriate text color for a given background color
function getTextColor(hexColor: string): '#FFFFFF' | '#000000' {
  // Remove # if present
  const hex = hexColor.replace('#', '');

  // Convert to RGB
  const r = parseInt(hex.substring(0, 2), 16);
  const g = parseInt(hex.substring(2, 4), 16);
  const b = parseInt(hex.substring(4, 6), 16);

  // Calculate relative luminance (WCAG formula)
  const luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255;

  // Use black text for bright backgrounds, white for dark
  return luminance > 0.5 ? '#000000' : '#FFFFFF';
}

function generatePlatformTypes() {
  // Read JSON file
  const jsonPath = path.join(__dirname, '..', 'data', 'streaming-platforms.json');
  const jsonContent = fs.readFileSync(jsonPath, 'utf-8');
  const data: PlatformsJSON = JSON.parse(jsonContent);

  // Filter to only RTMP/RTMPS platforms with "append" or "in_url_template" placement
  const services = data.services.filter(service => {
    const isRTMP = service.defaultUrl.startsWith('rtmp://') || service.defaultUrl.startsWith('rtmps://');
    const isSupported = service.streamKeyPlacement === 'append' || service.streamKeyPlacement === 'in_url_template';
    return isRTMP && isSupported;
  });

  if (services.length === 0) {
    throw new Error('No RTMP platforms found in JSON');
  }

  // Generate TypeScript code
  let output = `// Auto-generated from data/streaming-platforms.json
// DO NOT EDIT MANUALLY
// Run 'npm run generate:platforms' to regenerate this file

/**
 * Supported streaming platforms
 * All platforms use RTMP/RTMPS with either "append" or "in_url_template" stream key placement
 */
export type Platform =\n`;

  // Generate union type
  services.forEach((service, index) => {
    const isLast = index === services.length - 1;
    output += `  | '${service.name}'${isLast ? ';\n' : '\n'}`;
  });

  // Generate PLATFORMS constant
  output += `\n/**
 * Platform configuration mapping
 * Contains display names, colors, and default server URLs
 */
export const PLATFORMS: Record<Platform, {
  displayName: string;
  abbreviation: string;
  color: string;
  textColor: string;
  defaultServer: string;
  streamKeyPlacement: 'append' | 'in_url_template';
}> = {\n`;

  services.forEach((service, index) => {
    const isLast = index === services.length - 1;
    const displayName = service.displayName || service.name;
    const textColor = getTextColor(service.color);

    output += `  '${service.name}': {
    displayName: '${displayName}',
    abbreviation: '${service.abbreviation}',
    color: '${service.color}',
    textColor: '${textColor}',
    defaultServer: '${service.defaultUrl}',
    streamKeyPlacement: '${service.streamKeyPlacement}',
  }${isLast ? '\n' : ',\n'}`;
  });

  output += '};\n';

  // Write output file
  const outputPath = path.join(__dirname, '..', 'src-frontend', 'types', 'generated-platforms.ts');
  fs.writeFileSync(outputPath, output, 'utf-8');

  console.log(`✅ Generated ${services.length} platform types to ${outputPath}`);
}

// Run generator
try {
  generatePlatformTypes();
} catch (error) {
  console.error('❌ Failed to generate platform types:', error);
  process.exit(1);
}
