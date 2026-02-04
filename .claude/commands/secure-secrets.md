---
name: Secure Secrets Management
description: Use when handling API keys, passwords, encryption, environment variables, or any sensitive configuration. Ensures secrets are never exposed.
allowed-tools:
  - Read
  - Grep
  - Glob
  - Edit
  - Write
---

# Secure Secrets Management Guide

Apply these practices when handling sensitive data and credentials.

## Environment Variables

### Storage

```bash
# .env file (never commit!)
DATABASE_URL=postgres://user:pass@localhost/db
API_SECRET=sk_live_abc123
JWT_SIGNING_KEY=your-256-bit-secret
ENCRYPTION_KEY=32-byte-key-here
```

### .gitignore

```gitignore
# Always ignore
.env
.env.local
.env.*.local
*.pem
*.key
credentials.json
secrets/
```

### Loading

```python
# Python
from dotenv import load_dotenv
import os

load_dotenv()
api_key = os.environ.get('API_SECRET')

# Fail if missing required secrets
required = ['API_SECRET', 'DATABASE_URL']
missing = [k for k in required if not os.environ.get(k)]
if missing:
    raise RuntimeError(f"Missing required env vars: {missing}")
```

```javascript
// Node.js
require('dotenv').config();
const apiKey = process.env.API_SECRET;

// Validate at startup
const required = ['API_SECRET', 'DATABASE_URL'];
const missing = required.filter(k => !process.env[k]);
if (missing.length) {
  throw new Error(`Missing required env vars: ${missing.join(', ')}`);
}
```

## Client-Side Exposure Prevention

### Never Expose in Frontend

```javascript
// DANGEROUS - These get bundled into client code!
// React/Vite
const key = import.meta.env.VITE_SECRET_KEY  // Bad!

// Next.js
const key = process.env.NEXT_PUBLIC_SECRET   // Bad!

// These prefixes mean "public":
// VITE_*, NEXT_PUBLIC_*, REACT_APP_*, NUXT_PUBLIC_*
```

### Safe Pattern: Backend Proxy

```javascript
// Frontend calls your backend
const data = await fetch('/api/external-service');

// Backend makes the authenticated call
// server.js
app.get('/api/external-service', async (req, res) => {
  const response = await fetch('https://external-api.com/data', {
    headers: { 'Authorization': `Bearer ${process.env.API_SECRET}` }
  });
  res.json(await response.json());
});
```

### Audit for Exposure

```bash
# Search for exposed secrets in built JS
grep -r "sk_live\|api_key\|secret" dist/ build/

# Check for env vars in source
grep -r "VITE_\|NEXT_PUBLIC_\|REACT_APP_" src/ --include="*.js" --include="*.ts"
```

## Encryption

### Symmetric Encryption (AES-256-GCM)

```python
from cryptography.hazmat.primitives.ciphers.aead import AESGCM
import os

def encrypt(plaintext: bytes, key: bytes) -> bytes:
    """Encrypt with AES-256-GCM. Returns nonce + ciphertext."""
    if len(key) != 32:
        raise ValueError("Key must be 32 bytes")

    nonce = os.urandom(12)  # 96-bit nonce
    aesgcm = AESGCM(key)
    ciphertext = aesgcm.encrypt(nonce, plaintext, None)

    return nonce + ciphertext

def decrypt(data: bytes, key: bytes) -> bytes:
    """Decrypt AES-256-GCM. Expects nonce + ciphertext."""
    nonce = data[:12]
    ciphertext = data[12:]

    aesgcm = AESGCM(key)
    return aesgcm.decrypt(nonce, ciphertext, None)
```

```javascript
// Node.js
const crypto = require('crypto');

function encrypt(plaintext, key) {
  const nonce = crypto.randomBytes(12);
  const cipher = crypto.createCipheriv('aes-256-gcm', key, nonce);

  const encrypted = Buffer.concat([
    cipher.update(plaintext, 'utf8'),
    cipher.final()
  ]);
  const tag = cipher.getAuthTag();

  return Buffer.concat([nonce, tag, encrypted]);
}

function decrypt(data, key) {
  const nonce = data.slice(0, 12);
  const tag = data.slice(12, 28);
  const ciphertext = data.slice(28);

  const decipher = crypto.createDecipheriv('aes-256-gcm', key, nonce);
  decipher.setAuthTag(tag);

  return Buffer.concat([
    decipher.update(ciphertext),
    decipher.final()
  ]).toString('utf8');
}
```

### Key Derivation from Password

```python
from cryptography.hazmat.primitives.kdf.argon2 import Argon2id

def derive_key(password: str, salt: bytes = None) -> tuple[bytes, bytes]:
    """Derive a 256-bit key from password using Argon2id."""
    if salt is None:
        salt = os.urandom(16)

    kdf = Argon2id(
        salt=salt,
        length=32,
        iterations=3,
        lanes=4,
        memory_cost=65536,  # 64MB
    )
    key = kdf.derive(password.encode())

    return key, salt
```

### Encryption Rules

| Rule | Why |
|------|-----|
| Use authenticated encryption (GCM, ChaCha20-Poly1305) | Prevents tampering |
| Generate random IV/nonce per encryption | Prevents pattern analysis |
| Use KDF for password-derived keys | Prevents brute force |
| Store salt with ciphertext | Required for decryption |
| Use 256-bit keys minimum | Future-proof security |

### What NOT to Use

- ECB mode (patterns visible)
- CBC without HMAC (padding oracle)
- MD5/SHA1 for any security purpose
- DES/3DES (too small key)
- Custom encryption schemes

## API Key Security

### Storage in Database

```python
import hashlib
import secrets

def create_api_key(user_id):
    # Generate key
    raw_key = secrets.token_urlsafe(32)

    # Hash for storage (like password)
    key_hash = hashlib.sha256(raw_key.encode()).hexdigest()

    # Store hash in database
    db.api_keys.insert({
        'hash': key_hash,
        'user_id': user_id,
        'created_at': datetime.utcnow(),
        'last_used': None,
        'prefix': raw_key[:8]  # For identification
    })

    # Return raw key ONCE (user must save it)
    return raw_key

def verify_api_key(raw_key):
    key_hash = hashlib.sha256(raw_key.encode()).hexdigest()
    return db.api_keys.find_one({'hash': key_hash})
```

### Key Rotation

```python
def rotate_api_key(user_id, old_key):
    # Verify old key
    old_record = verify_api_key(old_key)
    if not old_record or old_record['user_id'] != user_id:
        raise AuthError()

    # Create new key
    new_key = create_api_key(user_id)

    # Mark old key as rotated (grace period)
    db.api_keys.update(
        {'hash': hashlib.sha256(old_key.encode()).hexdigest()},
        {'$set': {'rotated_at': datetime.utcnow()}}
    )

    # Delete old key after grace period (async job)
    schedule_deletion(old_record['id'], delay_hours=24)

    return new_key
```

## Secret Scanning

### Pre-Commit Hook

```yaml
# .pre-commit-config.yaml
repos:
  - repo: https://github.com/Yelp/detect-secrets
    rev: v1.4.0
    hooks:
      - id: detect-secrets
        args: ['--baseline', '.secrets.baseline']
```

### Patterns to Detect

```python
SECRET_PATTERNS = [
    r'(?i)api[_-]?key\s*[:=]\s*[\'"][^\'"]+[\'"]',
    r'(?i)secret\s*[:=]\s*[\'"][^\'"]+[\'"]',
    r'(?i)password\s*[:=]\s*[\'"][^\'"]+[\'"]',
    r'(?i)token\s*[:=]\s*[\'"][^\'"]+[\'"]',
    r'sk_live_[a-zA-Z0-9]+',  # Stripe
    r'ghp_[a-zA-Z0-9]+',  # GitHub
    r'AKIA[A-Z0-9]{16}',  # AWS
]
```

## Logging and Debugging

### Never Log Secrets

```python
# BAD
logger.info(f"Connecting with key: {api_key}")
logger.debug(f"Request body: {request.json}")  # May contain secrets

# GOOD
logger.info(f"Connecting with key: {api_key[:8]}...")
logger.debug(f"Request received for endpoint: {endpoint}")
```

### Mask in Error Messages

```python
class SecretMaskingFilter(logging.Filter):
    PATTERNS = [
        (r'sk_live_[a-zA-Z0-9]+', 'sk_live_***'),
        (r'password["\']?\s*[:=]\s*["\'][^"\']+["\']', 'password="***"'),
    ]

    def filter(self, record):
        msg = record.getMessage()
        for pattern, replacement in self.PATTERNS:
            msg = re.sub(pattern, replacement, msg)
        record.msg = msg
        record.args = ()
        return True
```

## Secrets in CI/CD

### GitHub Actions

```yaml
# .github/workflows/deploy.yml
jobs:
  deploy:
    steps:
      - name: Deploy
        env:
          API_KEY: ${{ secrets.API_KEY }}
        run: ./deploy.sh
```

### Never Commit

- `.env` files
- `credentials.json`
- `*.pem`, `*.key` files
- `terraform.tfvars` with secrets
- Kubernetes secrets YAML (unencrypted)

## Emergency Response

### If Secret is Exposed

1. **Immediately rotate** the compromised credential
2. **Audit logs** for unauthorized access
3. **Revoke** old credentials (don't just create new ones)
4. **Scan** for the secret in git history
5. **Notify** affected parties if data accessed

### Removing from Git History

```bash
# Using git-filter-repo (preferred)
pip install git-filter-repo
git filter-repo --invert-paths --path .env

# Force push (coordinate with team!)
git push --force --all
```

## Checklist

- [ ] Secrets in environment variables, not code
- [ ] .env files in .gitignore
- [ ] No secrets with VITE_/NEXT_PUBLIC_/REACT_APP_ prefix
- [ ] Encryption uses AES-256-GCM or ChaCha20-Poly1305
- [ ] Random IV/nonce per encryption
- [ ] Password-derived keys use Argon2id/bcrypt/scrypt
- [ ] API keys hashed in database
- [ ] Secrets masked in logs
- [ ] Pre-commit secret scanning enabled
- [ ] CI/CD uses secure secret storage
