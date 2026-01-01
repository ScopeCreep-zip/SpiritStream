# Build System Documentation

## Overview

MagillaStream uses a multi-stage build process combining TypeScript compilation, resource copying, and Electron packaging.

## Build Pipeline

```
┌─────────────────────────────────────────────────────────────────────┐
│                          Build Pipeline                              │
├──────────────┬──────────────┬──────────────┬───────────────────────┤
│    Clean     │   Compile    │    Copy      │      Package          │
│              │              │  Resources   │                       │
│  npm clean   │  npm compile │npm copy-res  │   npm pack            │
│              │              │              │                       │
│ rm dist/     │   tsc        │ copyRes.js   │ electron-builder      │
│ rm release/  │              │              │                       │
└──────────────┴──────────────┴──────────────┴───────────────────────┘
        │              │              │               │
        ▼              ▼              ▼               ▼
   Clean slate     dist/         dist/ with      release/
                 (compiled)     all resources   (packaged)
```

## NPM Scripts

### Available Commands

| Command | Description | Usage |
|---------|-------------|-------|
| `npm run clean` | Remove dist/ and release/ | Before fresh build |
| `npm run compile` | Compile TypeScript | Development |
| `npm run copy-resources` | Copy non-TS files | After compile |
| `npm run pack` | Package with electron-builder | Distribution |
| `npm run build` | Full build pipeline | Release build |
| `npm run dev` | Development mode | Development |
| `npm run electron` | Launch compiled app | Testing |
| `npm run start` | Direct Electron launch | Quick testing |

### Script Definitions

```json
{
  "scripts": {
    "clean": "node scripts/clean.js",
    "compile": "tsc",
    "copy-resources": "node scripts/copyResources.js",
    "pack": "electron-builder",
    "build": "npm run clean && npm run compile && npm run pack && npm run copy-resources",
    "dev": "npm run compile && npm run copy-resources && npm run electron",
    "electron": "electron dist/electron/main.js",
    "start": "electron ."
  }
}
```

## TypeScript Configuration

### tsconfig.json

```json
{
  "compilerOptions": {
    "target": "ESNext",
    "module": "CommonJS",
    "lib": ["ESNext"],
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "outDir": "./dist",
    "rootDir": "./src",
    "resolveJsonModule": true,
    "declaration": true,
    "declarationMap": true,
    "sourceMap": true
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules", "dist", "release"]
}
```

### Key Settings

| Setting | Value | Purpose |
|---------|-------|---------|
| target | ESNext | Latest JavaScript features |
| module | CommonJS | Node.js compatibility |
| strict | true | Full type checking |
| outDir | ./dist | Compilation output |
| rootDir | ./src | Source directory |
| sourceMap | true | Debugging support |

## Build Scripts

### Clean Script

**Location**: `scripts/clean.js`

```javascript
const fs = require('fs');
const path = require('path');

const dirsToClean = ['dist', 'release'];

dirsToClean.forEach(dir => {
  const fullPath = path.join(__dirname, '..', dir);
  if (fs.existsSync(fullPath)) {
    fs.rmSync(fullPath, { recursive: true, force: true });
    console.log(`Cleaned: ${dir}`);
  }
});
```

### Copy Resources Script

**Location**: `scripts/copyResources.js`

```javascript
const fs = require('fs');
const path = require('path');

const copies = [
  { from: 'config', to: 'dist/config' },
  { from: 'src/frontend', to: 'dist/frontend' },
  { from: 'resources', to: 'dist/resources' }
];

function copyDir(src, dest) {
  fs.mkdirSync(dest, { recursive: true });
  const entries = fs.readdirSync(src, { withFileTypes: true });

  for (const entry of entries) {
    const srcPath = path.join(src, entry.name);
    const destPath = path.join(dest, entry.name);

    if (entry.isDirectory()) {
      copyDir(srcPath, destPath);
    } else {
      fs.copyFileSync(srcPath, destPath);
    }
  }
}

copies.forEach(({ from, to }) => {
  const srcPath = path.join(__dirname, '..', from);
  const destPath = path.join(__dirname, '..', to);

  if (fs.existsSync(srcPath)) {
    copyDir(srcPath, destPath);
    console.log(`Copied: ${from} → ${to}`);
  }
});
```

## Electron Builder Configuration

### package.json Build Config

```json
{
  "build": {
    "appId": "com.magillastream.app",
    "productName": "MagillaStream",
    "directories": {
      "output": "release"
    },
    "files": [
      "dist/**/*",
      "config/**/*",
      "src/frontend/**/*"
    ],
    "extraResources": [
      {
        "from": "resources/",
        "to": "resources/"
      }
    ],
    "win": {
      "target": "nsis",
      "icon": "resources/icons/icon.ico"
    },
    "mac": {
      "target": "dmg",
      "icon": "resources/icons/icon.icns"
    },
    "linux": {
      "target": "AppImage",
      "icon": "resources/icons"
    }
  }
}
```

### Build Output Structure

```
release/
├── win-unpacked/              # Unpacked Windows build
│   ├── MagillaStream.exe
│   ├── resources/
│   │   ├── app.asar           # Packaged application
│   │   └── resources/
│   │       └── ffmpeg/
│   └── ...
├── MagillaStream Setup.exe    # Windows installer
├── MagillaStream.dmg          # macOS disk image
└── MagillaStream.AppImage     # Linux AppImage
```

## Directory Structure

### Source (Before Build)

```
magillastream/
├── src/
│   ├── electron/
│   │   ├── main.ts
│   │   ├── ipcHandlers.ts
│   │   └── preload.ts
│   ├── models/
│   ├── utils/
│   ├── frontend/
│   │   └── index/
│   ├── shared/
│   └── types/
├── config/
│   └── encoders.conf
├── resources/
│   └── ffmpeg/
│       └── bin/
├── scripts/
├── package.json
└── tsconfig.json
```

### Distribution (After Build)

```
dist/
├── electron/
│   ├── main.js
│   ├── main.js.map
│   ├── ipcHandlers.js
│   ├── ipcHandlers.js.map
│   ├── preload.js
│   └── preload.js.map
├── models/
│   ├── Profile.js
│   ├── OutputGroup.js
│   ├── StreamTarget.js
│   └── Theme.js
├── utils/
│   ├── profileManager.js
│   ├── ffmpegHandler.js
│   ├── encryption.js
│   └── logger.js
├── shared/
│   └── interfaces.js
├── types/
│   └── preload.d.ts
├── config/                    # Copied
│   └── encoders.conf
├── frontend/                  # Copied
│   └── index/
│       ├── index.html
│       ├── index.js
│       └── index.css
└── resources/                 # Copied
    └── ffmpeg/
```

## Development Workflow

### Initial Setup

```bash
# Clone repository
git clone https://github.com/billboyles/magillastream.git
cd magillastream

# Install dependencies
npm install

# First build
npm run build
```

### Development Cycle

```bash
# Make changes to TypeScript files
# Then run development build
npm run dev

# Or for faster iteration:
npm run compile && npm run electron
```

### Production Build

```bash
# Full clean build
npm run build

# Output in release/ directory
```

## Platform-Specific Builds

### Windows

```bash
# Build Windows installer
npm run pack -- --win

# Output: release/MagillaStream Setup.exe
```

### macOS

```bash
# Build macOS DMG
npm run pack -- --mac

# Output: release/MagillaStream.dmg
```

### Linux

```bash
# Build Linux AppImage
npm run pack -- --linux

# Output: release/MagillaStream.AppImage
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| NODE_ENV | Environment mode | development |
| DEBUG | Enable debug logging | false |

## Troubleshooting

### Common Issues

**TypeScript compilation errors**:
```bash
# Check TypeScript version
npx tsc --version

# Run with verbose output
npx tsc --listFiles
```

**Missing dependencies**:
```bash
# Clean install
rm -rf node_modules
npm install
```

**Electron not starting**:
```bash
# Check main.js exists
ls dist/electron/main.js

# Run with debugging
DEBUG=electron* npm run electron
```

**FFmpeg not found**:
```bash
# Verify FFmpeg in resources
ls resources/ffmpeg/bin/

# Check after copy
ls dist/resources/ffmpeg/bin/
```
