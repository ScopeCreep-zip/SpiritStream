---
description: Review code for security issues
allowed-tools:
  - Read
  - Grep
  - Glob
---

Perform a security review of the codebase focusing on:

## Tauri Security
- [ ] Capabilities are minimal in `apps/tauri/src-tauri/capabilities/default.json`
- [ ] CSP headers configured in `apps/tauri/src-tauri/tauri.conf.json`
- [ ] No unnecessary Tauri permissions granted

## Server Security
- [ ] Token auth enforced when `SPIRITSTREAM_API_TOKEN` is set
- [ ] CORS configuration is appropriate (check `server/src/main.rs`)
- [ ] Cookie-based session auth is secure (HttpOnly, SameSite)
- [ ] Rate limiting middleware is active

## Path Traversal
- [ ] `validate_path_within_any()` used for all file operations
- [ ] File browser endpoints validate paths
- [ ] No unvalidated user paths reach the filesystem

## Sensitive Data
- [ ] `mask_sensitive()` / `redact_payload()` used before logging
- [ ] Stream keys not logged in plaintext
- [ ] Stream keys encrypted at rest when `encrypt_stream_keys` is true
- [ ] OAuth tokens handled securely

## Encryption
- [ ] AES-256-GCM for profile encryption
- [ ] Argon2id key derivation
- [ ] Random salt and nonce per encryption

Check files:
- `server/src/main.rs` — routes, middleware, CORS, auth
- `server/src/services/encryption.rs` — encryption implementation
- `server/src/services/path_validator.rs` — path validation
- `server/src/services/oauth.rs` — OAuth token handling
- `apps/tauri/src-tauri/capabilities/` — Tauri permissions
- `apps/tauri/src-tauri/tauri.conf.json` — CSP headers

Report any findings with severity and recommended fixes.
