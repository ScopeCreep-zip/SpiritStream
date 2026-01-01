# Security Documentation

## Overview

MagillaStream implements security at multiple levels: Electron process isolation, profile encryption, and secure IPC communication.

## Electron Security Model

### Context Isolation

Context isolation prevents the renderer process from directly accessing Node.js APIs or the main process.

```typescript
// main.ts
const mainWindow = new BrowserWindow({
  webPreferences: {
    contextIsolation: true,    // Required for security
    sandbox: true,             // Additional isolation
    nodeIntegration: false,    // Prevent direct Node access
    preload: path.join(__dirname, 'preload.js')
  }
});
```

### Security Hierarchy

```
┌─────────────────────────────────────────────────────────────────┐
│                        Main Process                              │
│                    (Full Node.js Access)                         │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                    Preload Script                            ││
│  │              (Limited, Controlled Bridge)                    ││
│  │                                                              ││
│  │  contextBridge.exposeInMainWorld('electronAPI', {           ││
│  │    // Only these methods are accessible                     ││
│  │    profileManager: { ... },                                 ││
│  │    ffmpegHandler: { ... }                                   ││
│  │  });                                                        ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Renderer Process                            │
│                   (Isolated, Web Context)                        │
│                                                                  │
│  // Can only access window.electronAPI                          │
│  // Cannot access require, process, fs, etc.                    │
└─────────────────────────────────────────────────────────────────┘
```

### Sandbox Mode

Sandbox mode provides additional OS-level isolation:

- Restricts file system access
- Limits network capabilities
- Prevents process spawning
- Isolates from other processes

## Profile Encryption

### Algorithm Details

| Component | Specification |
|-----------|---------------|
| Algorithm | AES-256-GCM |
| Mode | Galois/Counter Mode (Authenticated) |
| Key Length | 256 bits (32 bytes) |
| IV Length | 96 bits (12 bytes) |
| Auth Tag | 128 bits (16 bytes) |
| Salt Length | 128 bits (16 bytes) |

### Key Derivation

PBKDF2 (Password-Based Key Derivation Function 2):

| Parameter | Value |
|-----------|-------|
| Hash | SHA-256 |
| Iterations | 100,000 |
| Output Length | 32 bytes |

### Encryption Flow

```typescript
encrypt(plaintext: string, password: string): string {
  // 1. Generate cryptographically secure random salt
  const salt = crypto.randomBytes(16);

  // 2. Derive key using PBKDF2
  const key = crypto.pbkdf2Sync(
    password,           // Password from user
    salt,              // Random salt
    100000,            // Iterations
    32,                // Key length
    'sha256'           // Hash algorithm
  );

  // 3. Generate random initialization vector
  const iv = crypto.randomBytes(12);

  // 4. Create authenticated cipher
  const cipher = crypto.createCipheriv('aes-256-gcm', key, iv);

  // 5. Encrypt data
  const encrypted = Buffer.concat([
    cipher.update(plaintext, 'utf8'),
    cipher.final()
  ]);

  // 6. Get authentication tag (prevents tampering)
  const authTag = cipher.getAuthTag();

  // 7. Combine: salt + iv + authTag + encrypted
  const combined = Buffer.concat([salt, iv, authTag, encrypted]);

  // 8. Encode as Base64 for storage
  return combined.toString('base64');
}
```

### Decryption Flow

```typescript
decrypt(ciphertext: string, password: string): string {
  // 1. Decode from Base64
  const data = Buffer.from(ciphertext, 'base64');

  // 2. Extract components
  const salt = data.subarray(0, 16);
  const iv = data.subarray(16, 28);
  const authTag = data.subarray(28, 44);
  const encrypted = data.subarray(44);

  // 3. Derive same key using password + salt
  const key = crypto.pbkdf2Sync(password, salt, 100000, 32, 'sha256');

  // 4. Create decipher with auth tag
  const decipher = crypto.createDecipheriv('aes-256-gcm', key, iv);
  decipher.setAuthTag(authTag);

  // 5. Decrypt
  const decrypted = Buffer.concat([
    decipher.update(encrypted),
    decipher.final()
  ]);

  return decrypted.toString('utf8');
}
```

### Data Format

```
┌──────────────────────────────────────────────────────────────────┐
│                     Encrypted Profile Data                        │
├─────────┬─────────┬──────────┬──────────────────────────────────┤
│  Salt   │   IV    │ Auth Tag │         Encrypted Data           │
│ 16 bytes│ 12 bytes│ 16 bytes │          Variable                │
├─────────┴─────────┴──────────┴──────────────────────────────────┤
│                        Base64 Encoded                            │
└──────────────────────────────────────────────────────────────────┘
```

## Stream Key Protection

### In-Memory Handling

Stream keys are sensitive credentials. Best practices:

```javascript
// DON'T: Log stream keys
console.log(`Starting stream to ${target.url}/${target.streamKey}`);  // BAD

// DO: Mask stream keys in logs
console.log(`Starting stream to ${target.url}/****`);  // GOOD
```

### Profile Export

When exporting profiles for backup:

```typescript
export(): ExportedProfile {
  return {
    name: this.name,
    incomingUrl: this.incomingUrl,
    outputGroups: this.outputGroups.map(g => ({
      ...g.toDTO(),
      streamTargets: g.streamTargets.map(t => ({
        url: t.url,
        streamKey: '***REDACTED***',  // Never export stream keys
        port: t.port
      }))
    }))
  };
}
```

## IPC Security

### Input Validation

All IPC handlers validate incoming data:

```typescript
ipcMain.handle('profile:load', async (_, name: string, password?: string) => {
  // Validate name
  if (!name || typeof name !== 'string') {
    throw new Error('Invalid profile name');
  }

  // Sanitize name (prevent path traversal)
  const sanitizedName = path.basename(name);
  if (sanitizedName !== name) {
    throw new Error('Invalid profile name');
  }

  // Validate password if provided
  if (password !== undefined && typeof password !== 'string') {
    throw new Error('Invalid password');
  }

  return ProfileManager.getInstance().load(sanitizedName, password);
});
```

### Path Traversal Prevention

```typescript
getProfilePath(name: string): string {
  // Prevent directory traversal attacks
  const safeName = name.replace(/[^a-zA-Z0-9-_]/g, '_');
  return path.join(this.profilesDir, `${safeName}.json`);
}
```

## File System Security

### User Data Isolation

All user data is stored in the application-specific directory:

```typescript
const userDataPath = app.getPath('userData');
// Windows: C:\Users\<user>\AppData\Roaming\MagillaStream
// macOS: ~/Library/Application Support/MagillaStream
// Linux: ~/.config/MagillaStream
```

### Directory Structure

```
{userData}/
├── profiles/          # Encrypted profile files
│   ├── MyProfile.json
│   └── ...
└── logs/             # Application logs
    ├── app.log
    └── ffmpeg.log
```

### File Permissions

The application creates files with default user permissions. Sensitive files should be readable only by the owner.

## Content Security Policy

### CSP Headers

```html
<meta http-equiv="Content-Security-Policy" content="
  default-src 'self';
  script-src 'self';
  style-src 'self' 'unsafe-inline';
  img-src 'self' data:;
  connect-src 'self';
  font-src 'self';
  object-src 'none';
  base-uri 'self';
  form-action 'self';
">
```

### CSP Rules

| Directive | Value | Purpose |
|-----------|-------|---------|
| default-src | 'self' | Default to same-origin |
| script-src | 'self' | Only local scripts |
| style-src | 'self' 'unsafe-inline' | Local + inline styles |
| connect-src | 'self' | Same-origin connections |
| object-src | 'none' | No plugins |

## Security Checklist

### Development

- [ ] Context isolation enabled
- [ ] Node integration disabled
- [ ] Sandbox mode enabled
- [ ] CSP headers configured
- [ ] Input validation on all IPC handlers
- [ ] Path traversal prevention
- [ ] No sensitive data in logs

### Encryption

- [ ] Strong key derivation (PBKDF2, 100k iterations)
- [ ] Authenticated encryption (GCM mode)
- [ ] Random salt per encryption
- [ ] Random IV per encryption
- [ ] Secure memory handling

### Stream Keys

- [ ] Never logged in plaintext
- [ ] Redacted in exports
- [ ] Encrypted when saved
- [ ] Cleared from memory after use

## Threat Model

### Threats Addressed

| Threat | Mitigation |
|--------|------------|
| XSS in renderer | Context isolation, CSP |
| Main process access | Sandbox, preload bridge |
| Profile theft | AES-256-GCM encryption |
| Stream key exposure | Encryption, log masking |
| Path traversal | Input sanitization |
| Credential interception | RTMPS support (optional) |

### Out of Scope

| Threat | Reason |
|--------|--------|
| Physical access | OS responsibility |
| Memory forensics | OS responsibility |
| Network MITM | RTMP protocol limitation |
| Keyloggers | OS responsibility |
