---
name: Secure Input/Output
description: Use when handling user input, rendering content, or implementing forms. Covers XSS prevention, CSRF protection, and input validation.
allowed-tools:
  - Read
  - Grep
  - Glob
  - Edit
  - Write
---

# Secure Input/Output Guide

Apply these practices when handling user input and rendering content.

## Cross-Site Scripting (XSS) Prevention

### Input Sources to Protect

**Direct Inputs:**
- Form fields (all text inputs)
- Search queries
- File names
- Rich text / WYSIWYG content
- Comments, reviews, messages

**Indirect Inputs:**
- URL parameters and query strings
- URL fragments (hash values)
- HTTP headers (Referer, User-Agent if displayed)
- Third-party API data
- WebSocket messages
- postMessage from iframes
- LocalStorage/SessionStorage values

**Often Overlooked:**
- Error messages reflecting input
- PDF generators accepting HTML
- Email templates with user data
- Log viewers
- JSON responses rendered as HTML
- SVG uploads (can contain JavaScript)
- Markdown rendering (if allowing raw HTML)

### Output Encoding by Context

| Context | Encoding | Example |
|---------|----------|---------|
| HTML body | HTML entity | `<` → `&lt;` |
| HTML attribute | Attribute encode | `"` → `&quot;` |
| JavaScript | JS escape | `'` → `\'` |
| URL parameter | URL encode | ` ` → `%20` |
| CSS | CSS escape | `\` → `\\` |

**Framework Auto-Escaping:**
```jsx
// React - auto-escaped
<div>{userInput}</div>  // Safe

// React - DANGEROUS
<div dangerouslySetInnerHTML={{__html: userInput}} />  // XSS!
```

```vue
<!-- Vue - auto-escaped -->
<div>{{ userInput }}</div>  <!-- Safe -->

<!-- Vue - DANGEROUS -->
<div v-html="userInput"></div>  <!-- XSS! -->
```

### Content Security Policy (CSP)

**Recommended Policy:**
```
Content-Security-Policy:
  default-src 'self';
  script-src 'self';
  style-src 'self' 'unsafe-inline';
  img-src 'self' data: https:;
  font-src 'self';
  connect-src 'self' https://api.yourdomain.com;
  frame-ancestors 'none';
  base-uri 'self';
  form-action 'self';
  report-uri /csp-report;
```

**CSP Rules:**
- Avoid `'unsafe-inline'` for scripts
- Avoid `'unsafe-eval'`
- Use nonces for inline scripts: `script-src 'nonce-{random}'`
- Report violations to monitor attacks

### HTML Sanitization

```javascript
// Using DOMPurify
import DOMPurify from 'dompurify';

const clean = DOMPurify.sanitize(dirty, {
  ALLOWED_TAGS: ['b', 'i', 'em', 'strong', 'a', 'p', 'br'],
  ALLOWED_ATTR: ['href', 'title'],
  ALLOW_DATA_ATTR: false
});
```

---

## CSRF Protection

### Endpoints Requiring Protection

**All State-Changing Requests:**
- POST, PUT, PATCH, DELETE
- Any GET that changes state (fix these!)
- File uploads
- Settings changes
- Payments/transactions

**Pre-Authentication Actions:**
- Login (prevent login CSRF)
- Signup
- Password reset/change
- OAuth callbacks

### CSRF Token Implementation

```python
# Generate token
import secrets
csrf_token = secrets.token_urlsafe(32)
session['csrf_token'] = csrf_token

# Validate token
def validate_csrf(request):
    session_token = session.get('csrf_token')
    request_token = request.headers.get('X-CSRF-Token') or request.form.get('csrf_token')

    if not session_token or not request_token:
        raise CSRFError()

    if not secrets.compare_digest(session_token, request_token):
        raise CSRFError()
```

**Frontend Usage:**
```javascript
// Include in requests
fetch('/api/action', {
  method: 'POST',
  headers: {
    'X-CSRF-Token': document.querySelector('meta[name="csrf-token"]').content
  },
  body: JSON.stringify(data)
});
```

### SameSite Cookies

```
Set-Cookie: session=abc; SameSite=Strict; Secure; HttpOnly
```

| Value | Behavior |
|-------|----------|
| Strict | Never sent cross-site (best security) |
| Lax | Sent on top-level navigation (good balance) |
| None | Always sent (requires Secure flag) |

### CSRF Checklist

- [ ] Token is cryptographically random
- [ ] Token tied to user session
- [ ] Token validated on ALL state-changing requests
- [ ] Missing token = rejected (not skipped)
- [ ] Token regenerated on auth state change
- [ ] SameSite cookie attribute set
- [ ] Secure and HttpOnly flags on session cookie

---

## Input Validation

### Server-Side Validation (Required)

```python
from pydantic import BaseModel, validator, EmailStr
from typing import Optional

class UserInput(BaseModel):
    email: EmailStr
    name: str
    age: Optional[int] = None

    @validator('name')
    def name_valid(cls, v):
        if len(v) < 1 or len(v) > 100:
            raise ValueError('Name must be 1-100 characters')
        return v.strip()

    @validator('age')
    def age_valid(cls, v):
        if v is not None and (v < 0 or v > 150):
            raise ValueError('Invalid age')
        return v
```

### Validation Rules by Type

| Type | Validation |
|------|------------|
| Email | Regex + length limit |
| URL | Parse, whitelist schemes (http/https) |
| Integer | Type check, range bounds |
| String | Length limits, character whitelist if applicable |
| Date | Parse to date object, validate range |
| File | Extension, MIME, size, content validation |
| Phone | Parse with library, validate format |

### Dangerous Patterns to Block

```python
# Block path traversal
if '..' in user_input or user_input.startswith('/'):
    raise ValidationError()

# Block null bytes
if '\x00' in user_input:
    raise ValidationError()

# Block control characters
import re
if re.search(r'[\x00-\x1f\x7f]', user_input):
    raise ValidationError()
```

---

## Open Redirect Prevention

### Vulnerable Pattern
```python
# VULNERABLE
redirect_url = request.args.get('next')
return redirect(redirect_url)
```

### Secure Patterns

**1. Allowlist:**
```python
ALLOWED_REDIRECTS = {
    'dashboard': '/dashboard',
    'profile': '/profile',
    'settings': '/settings'
}

def safe_redirect(key):
    url = ALLOWED_REDIRECTS.get(key)
    if not url:
        url = '/dashboard'
    return redirect(url)
```

**2. Relative URLs Only:**
```python
from urllib.parse import urlparse

def safe_redirect(url):
    parsed = urlparse(url)
    # Must be relative (no scheme or netloc)
    if parsed.scheme or parsed.netloc:
        return redirect('/dashboard')
    # Must start with /
    if not url.startswith('/'):
        return redirect('/dashboard')
    # Block // (protocol-relative)
    if url.startswith('//'):
        return redirect('/dashboard')
    return redirect(url)
```

### Bypass Techniques to Block

| Bypass | Example |
|--------|---------|
| @ symbol | `https://legit.com@evil.com` |
| Protocol-relative | `//evil.com` |
| Backslash | `https://legit.com\@evil.com` |
| URL encoding | `%2f%2fevil.com` |
| JavaScript protocol | `javascript:alert(1)` |
| Data URL | `data:text/html,<script>...` |

---

## Security Headers

```
Content-Security-Policy: [see above]
X-Content-Type-Options: nosniff
X-Frame-Options: DENY
Referrer-Policy: strict-origin-when-cross-origin
Permissions-Policy: geolocation=(), microphone=(), camera=()
```

## Checklist

- [ ] All user input validated server-side
- [ ] Output encoded for context (HTML, JS, URL, CSS)
- [ ] CSP header configured
- [ ] CSRF tokens on all state-changing endpoints
- [ ] SameSite cookies enabled
- [ ] Open redirects use allowlist or validated relative URLs
- [ ] Rich text sanitized with DOMPurify or equivalent
- [ ] Security headers set on all responses
