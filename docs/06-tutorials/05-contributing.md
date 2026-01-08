# Contributing

[Documentation](../README.md) > [Tutorials](./README.md) > Contributing

---

This guide covers setting up a development environment, understanding the codebase structure, and contributing code to SpiritStream. Whether you're fixing a bug or adding a feature, this document will help you get started.

---

## Prerequisites

Before contributing, you should have:

- Familiarity with Git and GitHub workflows
- Basic knowledge of TypeScript and React
- Basic knowledge of Rust (for backend changes)
- A GitHub account

## What You'll Learn

By the end of this guide, you will be able to:

1. Set up the complete development environment
2. Navigate the project structure
3. Run SpiritStream in development mode
4. Follow code style guidelines
5. Submit pull requests

---

## Development Environment Setup

### Required Tools

Install the following before starting:

| Tool | Version | Purpose |
|------|---------|---------|
| **Node.js** | 18+ | Frontend build and tooling |
| **Rust** | 1.77+ | Backend compilation |
| **Tauri CLI** | 2.x | Desktop app framework |
| **Git** | Latest | Version control |

### Install Rust

```bash
# macOS/Linux
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Windows (download installer from rustup.rs)
# Or use winget:
winget install Rustlang.Rust.GNU
```

Verify installation:

```bash
rustc --version
cargo --version
```

### Install Node.js

Use a version manager for easier updates:

```bash
# Using nvm (macOS/Linux)
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash
nvm install 20
nvm use 20

# Using fnm (cross-platform)
cargo install fnm
fnm install 20
fnm use 20

# Or download directly from nodejs.org
```

### Install Tauri CLI

```bash
cargo install tauri-cli
```

### Clone the Repository

```bash
git clone https://github.com/ScopeCreep-zip/SpiritStream.git
cd SpiritStream
```

### Install Dependencies

```bash
# Install frontend dependencies
npm install

# Rust dependencies are handled automatically by Cargo
```

---

## Project Structure

```
SpiritStream/
├── src-frontend/              # React frontend
│   ├── components/            # React components
│   │   ├── ui/               # Base UI components
│   │   ├── layout/           # Layout components
│   │   ├── profile/          # Profile management
│   │   ├── stream/           # Streaming controls
│   │   └── settings/         # Settings panels
│   ├── hooks/                # Custom React hooks
│   ├── stores/               # Zustand state stores
│   ├── lib/                  # Utilities and API wrapper
│   ├── types/                # TypeScript type definitions
│   ├── locales/              # i18n translation files
│   └── styles/               # CSS and design tokens
│
├── src-tauri/                 # Rust backend
│   ├── src/
│   │   ├── commands/         # Tauri command handlers
│   │   ├── services/         # Business logic
│   │   ├── models/           # Data structures
│   │   └── utils/            # Helper functions
│   ├── Cargo.toml            # Rust dependencies
│   └── tauri.conf.json       # Tauri configuration
│
├── docs/                      # Documentation
├── public/                    # Static assets
├── package.json              # Node.js config
├── vite.config.ts            # Vite bundler config
├── tailwind.config.js        # Tailwind CSS config
└── tsconfig.json             # TypeScript config
```

### Key Files

| File | Purpose |
|------|---------|
| `src-tauri/src/main.rs` | Application entry point |
| `src-frontend/App.tsx` | React root component |
| `src-frontend/stores/` | Application state management |
| `src-tauri/src/commands/` | IPC command handlers |
| `src-tauri/tauri.conf.json` | App configuration, permissions |

---

## Running in Development

### Start Development Server

```bash
npm run tauri dev
```

This command:

1. Starts the Vite dev server (frontend hot reload)
2. Compiles the Rust backend
3. Opens the application window

### Development URLs

| Service | URL |
|---------|-----|
| Frontend Dev Server | http://localhost:5173 |
| Tauri DevTools | Right-click → Inspect |

### Hot Reload

- **Frontend changes** — Automatically reloaded
- **Rust changes** — Triggers recompilation (takes a few seconds)
- **Tauri config changes** — Requires restart

---

## Code Style

### TypeScript/React

**Formatting:** We use Prettier with the project's `.prettierrc`:

```bash
npm run format        # Format all files
npm run format:check  # Check without modifying
```

**Linting:** ESLint catches common issues:

```bash
npm run lint          # Check for issues
npm run lint:fix      # Auto-fix where possible
```

**Conventions:**

```typescript
// Components: PascalCase, one per file
export function ProfileCard({ profile }: ProfileCardProps) {
  // ...
}

// Hooks: camelCase with 'use' prefix
export function useStreamStats() {
  // ...
}

// Types: PascalCase, interfaces for objects
interface ProfileCardProps {
  profile: Profile;
  onSelect?: (id: string) => void;
}

// Constants: UPPER_SNAKE_CASE
const MAX_RETRY_COUNT = 3;
```

### Rust

**Formatting:** Use rustfmt:

```bash
cd src-tauri
cargo fmt             # Format all files
cargo fmt -- --check  # Check without modifying
```

**Linting:** Clippy catches common issues:

```bash
cargo clippy          # Check for issues
cargo clippy --fix    # Auto-fix where possible
```

**Conventions:**

```rust
// Structs: PascalCase
pub struct ProfileManager {
    profiles_dir: PathBuf,
}

// Functions: snake_case
pub async fn load_profile(name: &str) -> Result<Profile, String> {
    // ...
}

// Constants: UPPER_SNAKE_CASE
const MAX_RETRIES: u32 = 3;

// Tauri commands: snake_case, async
#[tauri::command]
pub async fn get_all_profiles() -> Result<Vec<String>, String> {
    // ...
}
```

### Commits

Follow conventional commit format:

```
type(scope): description

[optional body]

[optional footer]
```

**Types:**

| Type | Use For |
|------|---------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation changes |
| `style` | Code formatting (no logic change) |
| `refactor` | Code restructuring |
| `test` | Adding/modifying tests |
| `chore` | Maintenance tasks |

**Examples:**

```bash
git commit -m "feat(streaming): add multi-bitrate output groups"
git commit -m "fix(profiles): handle encrypted profile load errors"
git commit -m "docs(tutorials): add first-stream guide"
```

---

## Making Changes

### Branch Naming

```
feature/description     # New features
fix/description        # Bug fixes
docs/description       # Documentation
refactor/description   # Code restructuring
```

**Examples:**

```bash
git checkout -b feature/hardware-encoder-detection
git checkout -b fix/stream-key-masking
git checkout -b docs/contributing-guide
```

### Development Workflow

```mermaid
%%{init: {'theme': 'base', 'themeVariables': {
  'primaryColor': '#3D3649',
  'primaryTextColor': '#F4F2F7',
  'primaryBorderColor': '#7C3AED',
  'lineColor': '#9489A8',
  'secondaryColor': '#251A33',
  'tertiaryColor': '#1A1225',
  'background': '#0F0A14',
  'mainBkg': '#1A1225',
  'nodeBorder': '#5E5472',
  'clusterBkg': '#251A33',
  'clusterBorder': '#3D3649',
  'titleColor': '#A78BFA',
  'edgeLabelBackground': '#1A1225',
  'textColor': '#F4F2F7'
}}}%%
flowchart TD
    A["Fork repository"] --> B["Create branch"]
    B --> C["Make changes"]
    C --> D["Run tests/lints"]
    D --> E{"Passes?"}
    E -->|No| C
    E -->|Yes| F["Commit changes"]
    F --> G["Push to fork"]
    G --> H["Open PR"]
    H --> I["Address feedback"]
    I --> J["Merge"]
```

### Adding a New Feature

1. **Create a branch:**
   ```bash
   git checkout -b feature/my-feature
   ```

2. **Make changes** following the code style guidelines

3. **Test your changes:**
   ```bash
   npm run tauri dev    # Manual testing
   npm run lint         # Frontend linting
   cd src-tauri && cargo clippy  # Backend linting
   ```

4. **Commit with conventional format:**
   ```bash
   git add .
   git commit -m "feat(scope): add my feature"
   ```

5. **Push and create PR:**
   ```bash
   git push origin feature/my-feature
   ```

---

## Pull Request Guidelines

### PR Title

Use the same format as commits:

```
feat(streaming): add multi-bitrate output groups
```

### PR Description

Include:

```markdown
## Summary
Brief description of changes.

## Changes
- Change 1
- Change 2

## Testing
How you tested the changes.

## Screenshots (if UI changes)
[Include screenshots]
```

### Review Checklist

Before requesting review:

- [ ] Code follows style guidelines
- [ ] `npm run lint` passes
- [ ] `cargo clippy` passes
- [ ] Manual testing completed
- [ ] Documentation updated (if needed)
- [ ] No console errors in development

---

## Testing

### Manual Testing

Most testing is currently manual:

1. Run `npm run tauri dev`
2. Test affected features
3. Check browser console for errors
4. Check Rust logs for backend issues

### Type Checking

```bash
# Frontend
npm run typecheck

# Backend
cd src-tauri && cargo check
```

### Building for Production

Test that production builds work:

```bash
npm run tauri build
```

---

## Common Tasks

### Adding a Tauri Command

1. **Define the command** in `src-tauri/src/commands/`:

   ```rust
   #[tauri::command]
   pub async fn my_command(param: String) -> Result<String, String> {
       // Implementation
       Ok("result".to_string())
   }
   ```

2. **Register in `main.rs`:**

   ```rust
   .invoke_handler(tauri::generate_handler![
       // ... existing commands
       commands::my_command,
   ])
   ```

3. **Call from frontend:**

   ```typescript
   import { invoke } from '@tauri-apps/api/core';

   const result = await invoke<string>('my_command', { param: 'value' });
   ```

### Adding a React Component

1. **Create the component** in appropriate directory:

   ```typescript
   // src-frontend/components/ui/MyComponent.tsx
   interface MyComponentProps {
     value: string;
   }

   export function MyComponent({ value }: MyComponentProps) {
     return <div className="...">{value}</div>;
   }
   ```

2. **Export from index** (if in a grouped folder):

   ```typescript
   // src-frontend/components/ui/index.ts
   export * from './MyComponent';
   ```

### Adding a Zustand Store

```typescript
// src-frontend/stores/myStore.ts
import { create } from 'zustand';

interface MyState {
  value: string;
  setValue: (value: string) => void;
}

export const useMyStore = create<MyState>((set) => ({
  value: '',
  setValue: (value) => set({ value }),
}));
```

---

## Getting Help

### Resources

| Resource | Link |
|----------|------|
| Tauri Documentation | https://tauri.app/v1/guides/ |
| React Documentation | https://react.dev |
| Rust Book | https://doc.rust-lang.org/book/ |
| Zustand Documentation | https://zustand-demo.pmnd.rs/ |

### Asking Questions

- **Bug reports:** Open a GitHub Issue with reproduction steps
- **Feature requests:** Open a GitHub Discussion first
- **Questions:** Check existing issues/discussions first

---

## License

By contributing to SpiritStream, you agree that your contributions will be licensed under the project's license.

---

**Related:** [System Overview](../01-architecture/01-system-overview.md) | [React Architecture](../03-frontend/01-react-architecture.md) | [Services Layer](../02-backend/02-services-layer.md)
