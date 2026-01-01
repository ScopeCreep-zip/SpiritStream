---
description: Review code for security issues
allowed-tools:
  - Read
  - Grep
  - Glob
---

Perform a security review of the codebase focusing on:

## Electron Security
- [ ] Context isolation is enabled
- [ ] Node integration is disabled
- [ ] Sandbox mode is enabled
- [ ] No `nodeIntegrationInWorker`
- [ ] No `webSecurity: false`

## IPC Security
- [ ] All IPC handlers validate input
- [ ] No path traversal vulnerabilities
- [ ] Sensitive data is not logged

## Encryption
- [ ] Strong key derivation (PBKDF2, high iterations)
- [ ] Random salt and IV per encryption
- [ ] Authenticated encryption (GCM mode)

## Stream Keys
- [ ] Not logged in plaintext
- [ ] Masked in user-visible output
- [ ] Encrypted when stored

Check files:
- `src/electron/main.ts` - Electron security settings
- `src/electron/ipcHandlers.ts` - IPC handler validation
- `src/utils/encryption.ts` - Encryption implementation
- `src/utils/logger.ts` - Logging (check for sensitive data)

Report any findings with severity and recommended fixes.
