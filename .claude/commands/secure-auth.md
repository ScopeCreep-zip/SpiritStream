---
name: Secure Authentication
description: Use when implementing login, signup, sessions, JWT, OAuth, or password reset flows. Ensures authentication security best practices.
allowed-tools:
  - Read
  - Grep
  - Glob
  - Edit
  - Write
---

# Secure Authentication Guide

Apply these practices when implementing authentication systems.

## Password Security

**Requirements:**
- Minimum 8 characters (12+ recommended)
- No maximum length (or very high: 128+ chars)
- Allow all characters including unicode
- Don't require specific character types (complexity rules hurt more than help)

**Storage - Use Only:**
- Argon2id (preferred)
- bcrypt (cost factor 12+)
- scrypt

**Never Use:** MD5, SHA1, SHA256 alone, or any unsalted hash

```python
# Example: Argon2id
from argon2 import PasswordHasher
ph = PasswordHasher(time_cost=3, memory_cost=65536, parallelism=4)
hash = ph.hash(password)
ph.verify(hash, password)  # Raises exception on mismatch
```

## Session Management

**Session Token Requirements:**
- Cryptographically random (256+ bits entropy)
- Regenerate on privilege change (login, logout, role change)
- Set expiration (absolute and idle timeout)
- Invalidate on logout (server-side)

**Cookie Security:**
```
Set-Cookie: session=<token>;
  HttpOnly;           # Prevent XSS access
  Secure;             # HTTPS only
  SameSite=Strict;    # CSRF protection
  Path=/;             # Scope appropriately
  Max-Age=3600        # Expiration
```

**Session Fixation Prevention:**
- Always regenerate session ID after successful login
- Don't accept session IDs from URL parameters
- Invalidate old session on new login

## JWT Security

**Token Structure:**
```
Header.Payload.Signature
```

**Critical Rules:**

| Rule | Why |
|------|-----|
| Validate signature FIRST | Prevents algorithm confusion attacks |
| Reject `alg: none` | Unsigned tokens are forgery |
| Whitelist algorithms | Only accept expected alg (e.g., RS256) |
| Validate `iss`, `aud`, `exp` | Prevents token reuse across services |
| Use short expiration | 15 min access, 7 day refresh typical |
| Store refresh tokens securely | HttpOnly cookie or secure storage |

**Algorithm Confusion Attack:**
```python
# VULNERABLE - accepts any algorithm
jwt.decode(token, secret, algorithms=jwt.get_unverified_header(token)['alg'])

# SECURE - whitelist algorithms
jwt.decode(token, public_key, algorithms=['RS256'])
```

**Refresh Token Rotation:**
- Issue new refresh token on each use
- Invalidate old refresh token immediately
- Detect reuse (indicates token theft)

**JWK/JWKS Validation:**
- Pin expected `kid` (key ID) values
- Validate JWKS endpoint is your own
- Cache JWKS with short TTL

## OAuth 2.0 / OIDC

**Authorization Code Flow (Required for Server Apps):**
1. Generate cryptographic `state` parameter
2. Use PKCE (`code_verifier` + `code_challenge`)
3. Validate `state` on callback
4. Exchange code server-side only
5. Validate `id_token` signature and claims

**PKCE Implementation:**
```python
import secrets
import hashlib
import base64

code_verifier = secrets.token_urlsafe(32)
code_challenge = base64.urlsafe_b64encode(
    hashlib.sha256(code_verifier.encode()).digest()
).rstrip(b'=').decode()
# Use code_challenge in auth request, code_verifier in token exchange
```

**OAuth Security Checklist:**
- [ ] State parameter is cryptographically random
- [ ] State is validated on callback (timing-safe comparison)
- [ ] PKCE is used (even for confidential clients)
- [ ] Redirect URI is exact match (no wildcards)
- [ ] Token exchange happens server-side
- [ ] Access tokens are not exposed to frontend (use BFF pattern)

## Multi-Factor Authentication

**TOTP Implementation:**
- Use 6-8 digit codes
- 30-second time step
- Allow 1 step clock skew
- Rate limit verification attempts
- Provide backup codes (one-time use, stored hashed)

**Recovery Codes:**
- Generate 10 single-use codes
- Store hashed (like passwords)
- Mark as used immediately on verification
- Allow regeneration (invalidates old codes)

## Account Security

**Brute Force Prevention:**
- Rate limit login attempts per account
- Rate limit by IP for credential stuffing
- Implement exponential backoff
- Consider CAPTCHA after N failures
- Don't reveal if account exists

**Account Enumeration Prevention:**
```python
# VULNERABLE
if not user_exists(email):
    return "User not found"
if not password_matches:
    return "Wrong password"

# SECURE
if not user_exists(email) or not password_matches(email, password):
    return "Invalid credentials"  # Same message, same timing
```

**Timing Attack Prevention:**
- Use constant-time comparison for secrets
- Ensure same response time for valid/invalid users

## Security Headers for Auth Pages

```
Strict-Transport-Security: max-age=31536000; includeSubDomains
Cache-Control: no-store
Pragma: no-cache
X-Content-Type-Options: nosniff
```

## Checklist

- [ ] Passwords hashed with Argon2id/bcrypt/scrypt
- [ ] Session regenerated on login
- [ ] JWT algorithm whitelisted
- [ ] JWT expiration validated
- [ ] OAuth state parameter validated
- [ ] PKCE implemented for OAuth
- [ ] Rate limiting on auth endpoints
- [ ] Same response for valid/invalid accounts
- [ ] MFA recovery codes hashed
- [ ] Secure cookie flags set
