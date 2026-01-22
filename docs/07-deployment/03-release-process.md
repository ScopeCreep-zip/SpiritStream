# Release Process

[Documentation](../README.md) > [Deployment](./README.md) > Release Process

---

This document outlines the release process for SpiritStream, from version preparation to distribution.

---

## Version Numbering

SpiritStream follows [Semantic Versioning](https://semver.org/):

```
MAJOR.MINOR.PATCH

1.0.0  - Initial release
1.1.0  - New features, backwards compatible
1.1.1  - Bug fixes
2.0.0  - Breaking changes
```

### Pre-release Versions

```
1.0.0-alpha.1  - Early testing
1.0.0-beta.1   - Feature complete, testing
1.0.0-rc.1     - Release candidate
```

---

## Release Checklist

### Pre-Release

- [ ] All tests passing
- [ ] Changelog updated
- [ ] Version numbers updated
- [ ] Documentation current
- [ ] No critical issues open

### Version Files

Update version in:

```
server/Cargo.toml                       → version = "1.0.0"
apps/desktop/src-tauri/Cargo.toml       → version = "1.0.0"
apps/desktop/src-tauri/tauri.conf.json  → "version": "1.0.0"
package.json                            → "version": "1.0.0"
```

### Changelog

```markdown
# Changelog

## [1.1.0] - 2024-01-15

### Added
- Multi-destination streaming support
- NVENC hardware encoding

### Changed
- Improved stream stability
- Updated UI components

### Fixed
- Connection timeout issues
- Profile encryption bug

### Security
- Updated dependencies
```

---

## Build Process

### 1. Clean Build

```bash
# Clean previous builds
rm -rf apps/desktop/src-tauri/target/release
rm -rf server/target/release
rm -rf apps/web/dist

# Fresh install
pnpm install
pnpm install --frozen-lockfile
>>>>>>> origin/main
```

### 2. Run Tests

```bash
# Frontend tests
pnpm test

# Rust tests
cargo test --manifest-path server/Cargo.toml
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml

# Type checking
pnpm typecheck
pnpm run typecheck
>>>>>>> origin/main
```

### 3. Build All Platforms

```bash
# Build for current platform
pnpm build:desktop
pnpm run tauri build
>>>>>>> origin/main

# Or use CI/CD for all platforms
```

---

## CI/CD Pipeline

### GitHub Actions

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - platform: macos-latest
            target: x86_64-apple-darwin
          - platform: macos-latest
            target: aarch64-apple-darwin
          - platform: windows-latest
            target: x86_64-pc-windows-msvc

    runs-on: ${{ matrix.platform }}

    steps:
      - uses: actions/checkout@v4

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: '20'

      - name: Setup pnpm
        uses: pnpm/action-setup@v2
        with:
          version: 8
          cache: 'pnpm'
>>>>>>> origin/main

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Install Linux Dependencies
        if: matrix.platform == 'ubuntu-latest'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev

      - name: Install Dependencies
        run: pnpm install

      - name: Build
        run: pnpm tauri build --target ${{ matrix.target }}
        run: pnpm install --frozen-lockfile

      - name: Build
        run: pnpm run tauri build -- --target ${{ matrix.target }}
>>>>>>> origin/main

      - name: Upload Artifacts
        uses: actions/upload-artifact@v4
        with:
          name: binaries-${{ matrix.target }}
          path: |
            apps/desktop/src-tauri/target/${{ matrix.target }}/release/bundle/

  release:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - name: Download Artifacts
        uses: actions/download-artifact@v4

      - name: Create Release
        uses: softprops/action-gh-release@v1
        with:
          files: |
            binaries-*/msi/*.msi
            binaries-*/dmg/*.dmg
            binaries-*/appimage/*.AppImage
            binaries-*/deb/*.deb
          draft: true
          generate_release_notes: true
```

---

## Code Signing

### macOS Signing

```bash
# Set identity
export APPLE_SIGNING_IDENTITY="Developer ID Application: Name (TEAMID)"

# Build with signing
pnpm tauri build
pnpm run tauri build
>>>>>>> origin/main

# Notarize
xcrun notarytool submit \
  "target/release/bundle/dmg/SpiritStream.dmg" \
  --apple-id "$APPLE_ID" \
  --password "$APPLE_PASSWORD" \
  --team-id "$TEAM_ID" \
  --wait

# Staple
xcrun stapler staple "target/release/bundle/dmg/SpiritStream.dmg"
```

### Windows Signing

```bash
# Sign with signtool
signtool sign /f certificate.pfx /p password \
  /tr http://timestamp.digicert.com /td sha256 \
  /fd sha256 \
  "target/release/bundle/msi/SpiritStream.msi"
```

### Environment Variables

```yaml
# GitHub Secrets
APPLE_SIGNING_IDENTITY: Developer ID...
APPLE_ID: your@email.com
APPLE_PASSWORD: app-specific-password
APPLE_TEAM_ID: XXXXXXXXXX
WINDOWS_CERTIFICATE: base64-encoded-pfx
WINDOWS_CERTIFICATE_PASSWORD: password
```

---

## Release Notes

### Template

```markdown
# SpiritStream v1.1.0

## Highlights

Brief summary of major changes.

## What's New

### Features
- Feature description

### Improvements
- Improvement description

### Bug Fixes
- Fix description

## Download

| Platform | Download |
|----------|----------|
| Windows | [SpiritStream_1.1.0_x64-setup.exe](link) |
| macOS Intel | [SpiritStream_1.1.0_x64.dmg](link) |
| macOS Apple Silicon | [SpiritStream_1.1.0_aarch64.dmg](link) |
| Linux | [SpiritStream_1.1.0_amd64.AppImage](link) |

## System Requirements

- Windows 10+ / macOS 10.15+ / Ubuntu 20.04+
- FFmpeg (auto-download available)

## Checksums

```
SHA256:
abc123... SpiritStream_1.1.0_x64-setup.exe
def456... SpiritStream_1.1.0_x64.dmg
```
```

---

## Distribution

### GitHub Releases

1. Create tag: `git tag v1.1.0`
2. Push tag: `git push origin v1.1.0`
3. CI builds and creates draft release
4. Review and edit release notes
5. Publish release

### Update Channels

| Channel | Purpose |
|---------|---------|
| Stable | Production releases |
| Beta | Pre-release testing |
| Dev | Nightly builds (optional) |

---

## Post-Release

### Verification

- [ ] Download and install on each platform
- [ ] Verify basic functionality
- [ ] Check auto-update (if implemented)
- [ ] Monitor issue reports

### Announcements

- Update website/landing page
- Social media announcement
- Email newsletter (if applicable)
- Community Discord/forum

### Monitoring

- GitHub Issues for bug reports
- Crash reports (if telemetry enabled)
- User feedback channels

---

## Hotfix Process

For critical bugs:

1. Create hotfix branch: `git checkout -b hotfix/1.1.1`
2. Fix issue
3. Update version to 1.1.1
4. Create PR to main
5. After merge, tag and release

```bash
git checkout main
git pull
git tag v1.1.1
git push origin v1.1.1
```

---

## Rollback

If release has critical issues:

1. Unpublish release on GitHub
2. Tag previous version as latest
3. Communicate with users
4. Fix and re-release

```bash
# Delete problematic tag
git tag -d v1.1.0
git push origin :refs/tags/v1.1.0
```

---

## Automation Scripts

### Version Bump Script

```bash
#!/bin/bash
# scripts/bump-version.sh

VERSION=$1

if [ -z "$VERSION" ]; then
  echo "Usage: ./bump-version.sh 1.2.0"
  exit 1
fi

# Update Cargo.toml files
sed -i "s/^version = .*/version = \"$VERSION\"/" server/Cargo.toml
sed -i "s/^version = .*/version = \"$VERSION\"/" apps/desktop/src-tauri/Cargo.toml

# Update package.json
pnpm version $VERSION --no-git-tag-version

# Update tauri.conf.json
jq ".version = \"$VERSION\"" apps/desktop/src-tauri/tauri.conf.json > tmp.json
mv tmp.json apps/desktop/src-tauri/tauri.conf.json

echo "Version bumped to $VERSION"
```

### Release Script

```bash
#!/bin/bash
# scripts/release.sh

VERSION=$1

if [ -z "$VERSION" ]; then
  echo "Usage: ./release.sh 1.2.0"
  exit 1
fi

# Bump version
./scripts/bump-version.sh $VERSION

# Commit
git add -A
git commit -m "chore: release v$VERSION"

# Tag
git tag "v$VERSION"

# Push
git push origin main
git push origin "v$VERSION"

echo "Release v$VERSION initiated"
```

---

**Related:** [Building](./01-building.md) | [Platform Guides](./02-platform-guides.md)

