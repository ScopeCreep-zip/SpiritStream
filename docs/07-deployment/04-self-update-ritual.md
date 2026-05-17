# Self-update release ritual

[Documentation](../README.md) > [Deployment](./README.md) > Self-update release ritual

---

This document describes the exact steps a maintainer follows to ship a new SpiritStream release that the in-app self-updater can consume.

The architecture is **maintainer-signed, CI-built, GitHub-Releases-hosted**: CI builds the artifacts on every `v*` tag and uploads them as a **draft** release; the maintainer downloads the drafts, signs each locally with the offline updater key, generates the `latest.json` manifest, uploads it, then publishes. The private key never enters CI, so no paid GitHub features (Actions secrets, Encrypted Secrets, Vault, etc.) are required.

---

## One-time setup

These steps only run once. Re-run only if rotating the updater key.

### Generate the updater keypair

```bash
pnpm tauri signer generate -w ~/.spiritstream-updater.key
```

This produces:

- `~/.spiritstream-updater.key` — the **private** key. Keep it offline. Pick your own storage: encrypted home dir, password manager export, a passphrase-protected USB stick, a YubiKey-encrypted file. **Never commit, never put in CI secrets.**
- A printed **public key** — looks like a one-line base64 blob.

### Embed the public key in the app

Paste the printed pubkey into `apps/tauri/src-tauri/tauri.conf.json`:

```jsonc
{
  "plugins": {
    "updater": {
      "endpoints": [
        "https://github.com/ScopeCreep-zip/SpiritStream/releases/latest/download/latest.json"
      ],
      "pubkey": "YOUR_BASE64_PUBKEY_HERE",
      "windows": { "installMode": "passive" }
    }
  }
}
```

Commit and push. From this point, every build embeds the pubkey; the runtime updater verifies each `.sig` against it. **Rotating the key requires shipping a new app version with the new pubkey embedded before users can update further.**

---

## Per-release ritual

### 1. Bump FFmpeg pin (optional)

If this release bumps the bundled FFmpeg version:

```bash
# Refresh URLs + SHA-256 from upstream:
pnpm tsx scripts/bump-ffmpeg-pin.ts --to 7.1.2

# Inspect the diff:
git diff scripts/ffmpeg-pins.json

# Verify locally:
pnpm tsx scripts/fetch-bundled-ffmpeg.ts --force
ffmpeg-aarch64-apple-darwin -version   # or the target you built
```

If no FFmpeg bump, ensure `scripts/ffmpeg-pins.json` has **non-empty** `sha256` fields for every supported platform. Empty fields cause `fetch-bundled-ffmpeg.ts` to abort the build with a clear error. Run `pnpm tsx scripts/bump-ffmpeg-pin.ts --populate` to fill them.

### 2. Bump version + tag

Update the version string in all four files:

- `package.json` → `"version": "X.Y.Z"`
- `apps/tauri/src-tauri/tauri.conf.json` → `"version": "X.Y.Z"`
- `apps/tauri/src-tauri/Cargo.toml` → `version = "X.Y.Z"`
- `server/Cargo.toml` → `version = "X.Y.Z"`

The release workflow's `validate-version` job fails the build if any of these drift.

```bash
git commit -am "release vX.Y.Z"
git tag vX.Y.Z
git push --tags
```

### 3. Wait for CI to build the draft

CI matrix builds macOS (arm64 + x86_64), Windows x86_64, Linux x86_64 — uploads everything to a **draft** release at `https://github.com/ScopeCreep-zip/SpiritStream/releases/tag/vX.Y.Z`.

The Sigstore cosign attestation job runs automatically after the build and uploads `.cosign.sig` + `.cosign.crt` files for every artifact. These provenance attestations are **independent** of the maintainer's updater signing — different file extension, different trust path.

```bash
gh run watch   # monitor until both build + cosign jobs are green
```

### 4. Download the draft artifacts locally

```bash
mkdir release-staging && cd release-staging
gh release download vX.Y.Z
ls -la
# SpiritStream_X.Y.Z_aarch64.app.tar.gz
# SpiritStream_X.Y.Z_x64.app.tar.gz
# SpiritStream_X.Y.Z_amd64.AppImage
# SpiritStream_X.Y.Z_x64-setup.nsis.zip
# SpiritStream_X.Y.Z_x64_en-US.msi
# (plus the .dmg and .deb/.rpm files which are not updater-consumable)
# (plus *.cosign.sig and *.cosign.crt from CI)
```

### 5. Sign the updater-consumable artifacts locally

The updater consumes specific file shapes per platform. Sign each:

```bash
# macOS Apple Silicon
pnpm tauri signer sign -f SpiritStream_X.Y.Z_aarch64.app.tar.gz -k ~/.spiritstream-updater.key

# macOS Intel
pnpm tauri signer sign -f SpiritStream_X.Y.Z_x64.app.tar.gz -k ~/.spiritstream-updater.key

# Linux AppImage
pnpm tauri signer sign -f SpiritStream_X.Y.Z_amd64.AppImage -k ~/.spiritstream-updater.key

# Windows NSIS installer (preferred — passive install)
pnpm tauri signer sign -f SpiritStream_X.Y.Z_x64-setup.nsis.zip -k ~/.spiritstream-updater.key
```

Each `tauri signer sign` produces `<artifact>.sig` next to the input. You'll be prompted for the key's passphrase once per file. (Tip: `expect`-script the loop if signing all of them is tedious — but read each prompt; you want to *know* you're signing what you think you're signing.)

### 6. Generate `latest.json`

```bash
node ../scripts/build-updater-manifest.mjs \
  --version X.Y.Z \
  --release-tag vX.Y.Z \
  --notes-file ../CHANGELOG.md \
  --out latest.json
```

Inspect:

```bash
cat latest.json | jq .
# Verify: every platform key your release supports has a `signature` and `url`.
```

### 7. Upload the maintainer-signed artifacts

```bash
gh release upload vX.Y.Z *.sig latest.json --clobber
```

(`--clobber` overwrites if you're re-running. Useful if you find a signing typo.)

### 8. Publish

```bash
gh release edit vX.Y.Z --draft=false
```

The release is now live. `https://github.com/ScopeCreep-zip/SpiritStream/releases/latest/download/latest.json` resolves to this version's manifest. The in-app updater starts seeing the new version on its next check.

---

## What users see

- **macOS**, **Windows**, and **Linux AppImage** users: About → Updates button shows a checking spinner, then either "You are running the latest version" or "Update available: X.Y.Z" with release notes and a "Download and install" button. Signature verification happens before install — a tampered artifact or wrong-signed bytes throw an error that's surfaced in the UI *and* recorded into the HMAC-chained audit log (`AuditAction::AppUpdateSignatureFailed`).
- **Linux `.deb` / `.rpm`** users: About → Updates button is hidden, replaced with "Updates are managed by your distribution's package manager. Run `apt upgrade spiritstream` or `dnf upgrade spiritstream`." Detected at runtime via the `APPIMAGE` env var per [appimage.org's runtime contract](https://docs.appimage.org/packaging-guide/environment-variables.html).

---

## Failure modes

| Symptom | Cause | Fix |
|---|---|---|
| `latest.json` 404 from the updater | Release is still a draft, or tagged but not the most recent published | `gh release edit vX.Y.Z --draft=false` |
| User sees "Signature verification failed" | `.sig` content mismatched against the embedded pubkey | Re-sign with the right key, or check `tauri.conf.json` `plugins.updater.pubkey` is current |
| `tauri signer sign` errors with "no such key" | `~/.spiritstream-updater.key` was moved/deleted | Restore from your offline backup, OR rotate (generate new keypair, embed new pubkey, ship a transition release before old key is fully retired) |
| `bump-ffmpeg-pin.ts` says SHA-256 mismatch with BtbN's `checksums.sha256` | Upstream rebuilt the artifact in place | Re-run; if persistent, inspect BtbN's release log |
| `validate-version` fails on tag push | Version strings out of sync across the 4 files | Update each, force-push the tag if needed: `git tag -d vX.Y.Z && git push --delete origin vX.Y.Z && git tag vX.Y.Z && git push --tags` |

---

## Trust model recap

Each release has **two independent signature paths**, both verifiable by users:

| Path | What it attests | Where it lives | Verifier |
|---|---|---|---|
| **Maintainer updater key** (ed25519, offline) | "This artifact was approved for release by the SpiritStream maintainer" | `.sig` files committed to the GitHub Release; pubkey embedded in `tauri.conf.json` | `tauri-plugin-updater` at runtime; rejects unsigned/tampered updates |
| **Sigstore cosign** (keyless OIDC) | "This artifact was built by the SpiritStream release workflow on this tag at this commit" | `.cosign.sig` + `.cosign.crt` files committed to the GitHub Release | `cosign verify-blob` with the documented `--certificate-identity-regexp` |

A compromised CI cannot forge the maintainer signature. A compromised maintainer laptop cannot forge the cosign provenance. The two together establish "this binary was built by our CI on this tagged commit and approved by the maintainer holding the offline key" — neither party alone can ship a backdoored release.

---

## See also

- Plan: `/Users/kali/.claude/plans/wobbly-wobbling-matsumoto.md`
- Build script: `scripts/fetch-bundled-ffmpeg.ts`
- Pin file: `scripts/ffmpeg-pins.json`
- Pin-bump helper: `scripts/bump-ffmpeg-pin.ts`
- Updater manifest generator: `scripts/build-updater-manifest.mjs`
- Release workflow: `.github/workflows/release.yml`
- Updater plugin docs: https://v2.tauri.app/plugin/updater/
- Sigstore cosign: https://docs.sigstore.dev/cosign/overview/
