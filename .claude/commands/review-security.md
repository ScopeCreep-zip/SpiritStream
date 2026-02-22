---
description: Review code for security issues
allowed-tools:
  - Read
  - Grep
  - Glob
---

Perform a security review of the codebase focusing on:

## Tauri Security
- [ ] Context isolation is enabled
- [ ] CSP is properly configured in `apps/desktop/src-tauri/tauri.conf.json`
- [ ] Capabilities follow least-privilege (`apps/desktop/src-tauri/capabilities/`)
- [ ] No arbitrary shell execution exposed to frontend

## Backend Security (Axum)
- [ ] CORS is properly configured in `server/src/main.rs`
- [ ] Token authentication enforced when configured
- [ ] All route handlers validate input
- [ ] Path traversal prevention (`server/src/services/path_validator.rs`)
- [ ] No sensitive data in error responses

## Encryption
- [ ] AES-256-GCM authenticated encryption (`server/src/services/encryption.rs`)
- [ ] Argon2id key derivation with proper parameters
- [ ] Random salt and nonce per encryption
- [ ] Stream keys encrypted at rest

## Stream Keys & Secrets
- [ ] Not logged in plaintext
- [ ] Masked in user-visible output
- [ ] Encrypted when stored in profiles

## WebSocket Security
- [ ] Token validation on WS upgrade
- [ ] No sensitive data broadcast to all clients

Check files:
- `server/src/main.rs` — CORS, middleware, router
- `server/src/routes/` — Route handler validation
- `server/src/services/encryption.rs` — Encryption implementation
- `server/src/services/path_validator.rs` — Path traversal prevention
- `apps/desktop/src-tauri/tauri.conf.json` — CSP and security config
- `apps/desktop/src-tauri/capabilities/` — Permission definitions

Report any findings with severity and recommended fixes.
