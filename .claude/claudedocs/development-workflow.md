# Development Workflow

## Overview

This guide covers the development workflow for MagillaStream, including setup, testing, debugging, and contributing.

## Getting Started

### Prerequisites

| Requirement | Version | Purpose |
|-------------|---------|---------|
| Node.js | 18+ | Runtime |
| npm | 8+ | Package manager |
| Git | 2.0+ | Version control |
| FFmpeg | 4.0+ | Stream encoding |

### Initial Setup

```bash
# Clone the repository
git clone https://github.com/ScopeCreep-zip/SpiritStream.git
cd SpiritStream

# Install dependencies
npm install

# Verify FFmpeg is in resources/
ls resources/ffmpeg/bin/

# Build the project
npm run build

# Start the application
npm run dev
```

## Project Structure

```
magillastream/
├── src/                    # Source code
│   ├── electron/           # Main process
│   ├── models/            # Domain models
│   ├── utils/             # Services
│   ├── frontend/          # UI
│   ├── shared/            # Shared interfaces
│   └── types/             # TypeScript types
├── config/                # Configuration
├── resources/             # Static resources
├── scripts/               # Build scripts
├── docs/                  # Documentation
├── .claude/               # Claude Code config
│   └── claudedocs/        # Extended docs
├── dist/                  # Compiled output
└── release/               # Packaged builds
```

## Development Cycle

### Making Changes

1. **Create a feature branch**
   ```bash
   git checkout -b feature/my-feature
   ```

2. **Make your changes**
   - Edit TypeScript files in `src/`
   - Edit frontend files in `src/frontend/`
   - Add tests if applicable

3. **Compile and test**
   ```bash
   npm run dev
   ```

4. **Commit your changes**
   ```bash
   git add .
   git commit -m "Add feature description"
   ```

### Quick Iteration

For faster development cycles:

```bash
# Terminal 1: Watch TypeScript compilation
npx tsc --watch

# Terminal 2: Run the app (after each compilation)
npm run electron
```

## Code Organization

### Adding a New Model

1. Create model file in `src/models/`:
   ```typescript
   // src/models/NewModel.ts
   export class NewModel {
     private _id: string;
     private _name: string;

     constructor(name: string) {
       this._id = generateUUID();
       this._name = name;
     }

     get id(): string { return this._id; }
     get name(): string { return this._name; }
     set name(value: string) { this._name = value; }

     toDTO(): NewModelDTO {
       return {
         id: this._id,
         name: this._name
       };
     }

     static fromDTO(dto: NewModelDTO): NewModel {
       const model = new NewModel(dto.name);
       model._id = dto.id;
       return model;
     }
   }
   ```

2. Add DTO interface in `src/shared/interfaces.ts`:
   ```typescript
   export interface NewModelDTO {
     id: string;
     name: string;
   }
   ```

### Adding a New Service

1. Create service file in `src/utils/`:
   ```typescript
   // src/utils/newService.ts
   export class NewService {
     private static instance: NewService;

     private constructor() {}

     public static getInstance(): NewService {
       if (!NewService.instance) {
         NewService.instance = new NewService();
       }
       return NewService.instance;
     }

     public async doSomething(): Promise<void> {
       // Implementation
     }
   }
   ```

2. Register IPC handlers in `src/electron/ipcHandlers.ts`:
   ```typescript
   ipcMain.handle('newService:doSomething', async () => {
     return NewService.getInstance().doSomething();
   });
   ```

3. Expose in preload script `src/electron/preload.ts`:
   ```typescript
   newService: {
     doSomething: () => ipcRenderer.invoke('newService:doSomething')
   }
   ```

4. Update type definitions in `src/types/preload.d.ts`

### Adding Frontend Features

1. Update HTML structure in `src/frontend/index/index.html`
2. Add styles in `src/frontend/index/index.css`
3. Implement logic in `src/frontend/index/index.js`

## Debugging

### Main Process Debugging

```bash
# Run with Node.js inspector
node --inspect-brk ./node_modules/.bin/electron dist/electron/main.js
```

Open Chrome DevTools: `chrome://inspect`

### Renderer Process Debugging

1. Run the app normally
2. Press `Ctrl+Shift+I` (Windows/Linux) or `Cmd+Opt+I` (macOS) to open DevTools

### Logging

```typescript
// In main process
import { Logger } from '../utils/logger';
Logger.getInstance().debug('Debug message');
Logger.getInstance().info('Info message');
Logger.getInstance().error('Error message');

// In renderer process
window.electronAPI.logger.log('debug', 'Debug message');
```

### Log Files

```
{userData}/
└── logs/
    ├── app.log         # Application logs
    ├── ffmpeg.log      # FFmpeg output
    └── frontend.log    # Frontend logs
```

## Testing

### Manual Testing

1. **Profile Management**
   - Create new profile
   - Load existing profile
   - Save profile (with/without encryption)
   - Delete profile

2. **Stream Configuration**
   - Add/remove output groups
   - Add/remove stream targets
   - Change encoding settings

3. **Streaming**
   - Start stream (requires RTMP input)
   - Stop stream
   - Verify output to targets

### Test RTMP Setup

1. Install NGINX with RTMP module or use OBS to stream
2. Configure incoming URL: `rtmp://localhost:1935/live/test`
3. Use VLC or ffplay to verify output streams

## Common Tasks

### Adding a New Encoder

1. Update whitelist in `config/encoders.conf`:
   ```json
   {
     "video": ["...", "new_encoder"]
   }
   ```

2. Test encoder availability:
   ```bash
   ffmpeg -encoders | grep new_encoder
   ```

### Updating Dependencies

```bash
# Check for updates
npm outdated

# Update specific package
npm update <package-name>

# Update all packages
npm update
```

### Regenerating Types

After modifying interfaces:

```bash
npm run compile
```

## Git Workflow

### Branch Naming

| Type | Pattern | Example |
|------|---------|---------|
| Feature | `feature/<name>` | `feature/add-dark-mode` |
| Bug fix | `fix/<name>` | `fix/stream-timeout` |
| Refactor | `refactor/<name>` | `refactor/ipc-handlers` |
| Docs | `docs/<name>` | `docs/update-readme` |

### Commit Messages

```
<type>: <description>

[optional body]

[optional footer]
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`

Examples:
```
feat: add dark mode toggle to settings
fix: resolve stream key masking in logs
docs: update FFmpeg integration guide
refactor: extract encoder detection to separate module
```

### Pull Request Process

1. Create feature branch
2. Make changes
3. Compile and test locally
4. Push to remote
5. Create pull request
6. Address review feedback
7. Merge after approval

## Troubleshooting

### Build Fails

```bash
# Clean and rebuild
npm run clean
npm install
npm run build
```

### TypeScript Errors

```bash
# Check for type errors
npx tsc --noEmit

# View detailed errors
npx tsc --listFiles --noEmit
```

### Electron Won't Start

```bash
# Verify main.js exists
ls dist/electron/main.js

# Check for missing dependencies
npm install

# Run with verbose logging
DEBUG=electron* npm run electron
```

### FFmpeg Not Found

```bash
# Check FFmpeg location
ls resources/ffmpeg/bin/

# Verify it's executable
./resources/ffmpeg/bin/ffmpeg -version
```

## Performance Tips

1. **Use `--watch` for TypeScript**
   - Faster recompilation
   - Immediate error feedback

2. **Keep DevTools closed when not needed**
   - Reduces memory usage
   - Improves performance

3. **Profile with Chrome DevTools**
   - Memory snapshots
   - Performance timeline
   - CPU profiling

## IDE Setup

### VS Code Extensions

- TypeScript + JavaScript Language Features (built-in)
- ESLint
- Prettier
- GitLens

### Recommended Settings

```json
// .vscode/settings.json
{
  "typescript.tsdk": "node_modules/typescript/lib",
  "editor.formatOnSave": true,
  "editor.codeActionsOnSave": {
    "source.fixAll.eslint": true
  }
}
```

### Launch Configuration

```json
// .vscode/launch.json
{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "Debug Main Process",
      "type": "node",
      "request": "launch",
      "runtimeExecutable": "${workspaceFolder}/node_modules/.bin/electron",
      "args": ["${workspaceFolder}/dist/electron/main.js"],
      "sourceMaps": true,
      "outFiles": ["${workspaceFolder}/dist/**/*.js"]
    }
  ]
}
```
