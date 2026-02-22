# Security Model

## Remote Access Security

- Default binding: `localhost:8008` (remote access is opt-in)
- Token authentication: Bearer header + WebSocket query param
- Enforced only when token is configured
- UI serving disabled by default

## Tauri Security

- Capability-based permissions (default-deny model)
- CSP headers enforced in `apps/desktop/src-tauri/tauri.conf.json`
- IPC allowlist — only expose necessary commands
- No Node.js in renderer

## Profile Encryption

- AES-256-GCM encryption
- Argon2id key derivation (Rust, `server/src/services/encryption.rs`)
- Random salt and nonce per encryption
- Stream keys always encrypted at rest

## CORS Configuration

Backend (Rust/Axum) in `server/src/main.rs`:
```rust
let cors = CorsLayer::new()
    .allow_origin(Any)
    .allow_methods(Any)
    .allow_headers(Any);
```

Common issues:
- WebSocket blocked: Ensure `/ws` endpoint allows upgrade
- Preflight failures: Check OPTIONS requests are handled
- Credentials: Use `allow_credentials(true)` with specific origins

## CSP (Content Security Policy)

Tauri CSP in `apps/desktop/src-tauri/tauri.conf.json`:

| Directive | Required Values | Purpose |
|-----------|-----------------|---------|
| `default-src` | `'self'` | Base policy |
| `connect-src` | `'self' http://127.0.0.1:8008 ws://127.0.0.1:8008` | HTTP API + WebSocket |
| `img-src` | `'self' data: blob:` | Images, thumbnails, previews |
| `media-src` | `'self' blob:` | Video/audio streams |
| `style-src` | `'self' 'unsafe-inline'` | Tailwind + inline styles |

## Security Checklist

- Never log stream keys or tokens
- Sanitize all paths to prevent traversal attacks (`server/src/services/path_validator.rs`)
- Validate inputs on backend, trust nothing from frontend
- Mask stream keys with asterisks in logs
- No `webSecurity: false` in Tauri config
