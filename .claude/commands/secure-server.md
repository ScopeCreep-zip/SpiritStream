---
name: Secure Server-Side
description: Use when implementing server-side logic that processes external data, makes HTTP requests, handles file paths, or executes database queries. Covers SSRF, SQLi, XXE, path traversal.
allowed-tools:
  - Read
  - Grep
  - Glob
  - Edit
  - Write
---

# Secure Server-Side Guide

Apply these practices when implementing server-side logic.

## SQL Injection Prevention

### Parameterized Queries (Primary Defense)

```python
# VULNERABLE
query = f"SELECT * FROM users WHERE id = {user_id}"
cursor.execute(query)

# SECURE - Parameterized
cursor.execute("SELECT * FROM users WHERE id = %s", (user_id,))
```

```javascript
// VULNERABLE
const query = `SELECT * FROM users WHERE id = ${userId}`;

// SECURE - Parameterized
const query = 'SELECT * FROM users WHERE id = $1';
await pool.query(query, [userId]);
```

### ORM Safe Patterns

```python
# SQLAlchemy - Safe
User.query.filter_by(id=user_id).first()
User.query.filter(User.email == email).first()

# SQLAlchemy - DANGEROUS (raw SQL)
db.session.execute(f"SELECT * FROM users WHERE id = {user_id}")  # BAD!
db.session.execute("SELECT * FROM users WHERE id = :id", {"id": user_id})  # Safe
```

### Non-Parameterizable Inputs

**ORDER BY (whitelist only):**
```python
ALLOWED_SORT = {'name': 'name', 'date': 'created_at', 'price': 'price'}

def get_sorted(sort_by):
    column = ALLOWED_SORT.get(sort_by, 'created_at')
    return f"SELECT * FROM products ORDER BY {column}"  # Safe - whitelisted
```

**Table/Column Names (whitelist only):**
```python
ALLOWED_TABLES = {'users', 'products', 'orders'}

def query_table(table_name):
    if table_name not in ALLOWED_TABLES:
        raise ValueError("Invalid table")
    return f"SELECT * FROM {table_name}"
```

### IN Clauses

```python
# VULNERABLE
ids = ','.join(user_ids)
query = f"SELECT * FROM users WHERE id IN ({ids})"

# SECURE
placeholders = ','.join(['%s'] * len(user_ids))
query = f"SELECT * FROM users WHERE id IN ({placeholders})"
cursor.execute(query, user_ids)
```

---

## Server-Side Request Forgery (SSRF)

### Vulnerable Features

- Webhooks (user provides callback URL)
- URL previews / link unfurling
- PDF/image generation from URLs
- Import from URL
- Proxy functionality

### URL Validation

```python
import ipaddress
import socket
from urllib.parse import urlparse

def is_safe_url(url):
    try:
        parsed = urlparse(url)

        # Only allow http/https
        if parsed.scheme not in ('http', 'https'):
            return False

        # Resolve hostname
        hostname = parsed.hostname
        if not hostname:
            return False

        # Get IP address
        ip = socket.gethostbyname(hostname)
        ip_obj = ipaddress.ip_address(ip)

        # Block private/internal IPs
        if ip_obj.is_private or ip_obj.is_loopback or ip_obj.is_reserved:
            return False

        # Block cloud metadata IPs
        metadata_ips = ['169.254.169.254', '169.254.170.2']
        if ip in metadata_ips:
            return False

        return True
    except Exception:
        return False
```

### IP Bypass Techniques to Block

| Technique | Example |
|-----------|---------|
| Decimal IP | `http://2130706433` (127.0.0.1) |
| Octal IP | `http://0177.0.0.1` |
| Hex IP | `http://0x7f.0x0.0x1` |
| IPv6 localhost | `http://[::1]` |
| IPv6 mapped | `http://[::ffff:127.0.0.1]` |
| Short notation | `http://127.1` |
| DNS rebinding | Attacker DNS returns internal IP |

### DNS Rebinding Prevention

```python
import socket

def fetch_url_safely(url):
    parsed = urlparse(url)

    # Resolve DNS ONCE
    ip = socket.gethostbyname(parsed.hostname)

    # Validate IP is external
    if not is_external_ip(ip):
        raise SecurityError("Internal IP not allowed")

    # Make request using resolved IP
    # (Pin the IP so DNS can't change mid-request)
    session = requests.Session()
    session.mount('http://', ResolvedIPAdapter(ip))
    session.mount('https://', ResolvedIPAdapter(ip))

    return session.get(url, allow_redirects=False)
```

### Redirect Handling

```python
def fetch_with_redirect_check(url, max_redirects=3):
    for _ in range(max_redirects):
        if not is_safe_url(url):
            raise SecurityError("Unsafe URL")

        response = requests.get(url, allow_redirects=False)

        if response.status_code in (301, 302, 303, 307, 308):
            url = response.headers.get('Location')
            continue

        return response

    raise SecurityError("Too many redirects")
```

---

## XML External Entity (XXE)

### Disable External Entities

**Python (lxml):**
```python
from lxml import etree

parser = etree.XMLParser(
    resolve_entities=False,
    no_network=True,
    dtd_validation=False,
    load_dtd=False
)
tree = etree.parse(xml_file, parser)
```

**Python (defusedxml - recommended):**
```python
import defusedxml.ElementTree as ET
tree = ET.parse(xml_file)  # Safe by default
```

**Java:**
```java
DocumentBuilderFactory dbf = DocumentBuilderFactory.newInstance();
dbf.setFeature("http://apache.org/xml/features/disallow-doctype-decl", true);
dbf.setFeature("http://xml.org/sax/features/external-general-entities", false);
dbf.setFeature("http://xml.org/sax/features/external-parameter-entities", false);
dbf.setExpandEntityReferences(false);
```

**Node.js:**
```javascript
// Use libraries that disable DTD by default
// If using libxmljs:
const doc = libxmljs.parseXml(xml, { noent: false, dtdload: false });
```

### XXE in Office Documents

DOCX, XLSX, PPTX are ZIP files containing XML:
```python
import zipfile
import defusedxml.ElementTree as ET

def parse_docx_safely(docx_path):
    with zipfile.ZipFile(docx_path) as z:
        with z.open('word/document.xml') as f:
            # Use defusedxml for parsing
            tree = ET.parse(f)
```

---

## Path Traversal Prevention

### Secure Path Joining

```python
import os

def safe_path_join(base_dir, user_path):
    # Normalize and resolve both paths
    base = os.path.realpath(os.path.abspath(base_dir))
    target = os.path.realpath(os.path.abspath(os.path.join(base, user_path)))

    # Ensure target is under base
    if not target.startswith(base + os.sep) and target != base:
        raise SecurityError("Path traversal detected")

    return target
```

### Bypass Techniques to Block

| Technique | Example |
|-----------|---------|
| Basic traversal | `../../../etc/passwd` |
| URL encoding | `%2e%2e%2f` |
| Double encoding | `%252e%252e%252f` |
| Null byte | `../../../etc/passwd%00.jpg` |
| Backslash (Windows) | `..\..\..\windows\system32` |
| Mixed slashes | `..\/..\/..\/etc/passwd` |

### Filename Sanitization

```python
import re
import os

def sanitize_filename(filename):
    # Remove path separators
    filename = os.path.basename(filename)

    # Remove null bytes
    filename = filename.replace('\x00', '')

    # Whitelist safe characters
    filename = re.sub(r'[^a-zA-Z0-9._-]', '_', filename)

    # Prevent hidden files
    filename = filename.lstrip('.')

    # Limit length
    if len(filename) > 255:
        name, ext = os.path.splitext(filename)
        filename = name[:255-len(ext)] + ext

    return filename or 'unnamed'
```

---

## Command Injection Prevention

### Avoid Shell Commands

```python
# VULNERABLE
import os
os.system(f"convert {user_file} output.png")

# SECURE - Use library directly
from PIL import Image
img = Image.open(user_file)
img.save("output.png")
```

### If Shell Required

```python
import subprocess
import shlex

# VULNERABLE
subprocess.run(f"echo {user_input}", shell=True)

# SECURE - No shell, list args
subprocess.run(["echo", user_input], shell=False)

# If shell needed, use shlex.quote
subprocess.run(f"echo {shlex.quote(user_input)}", shell=True)
```

### Command Injection Characters

Block or escape: `; | & $ > < \` ' " ( ) { } [ ] ! \n \r`

---

## Logging Security

### What to Log

- Authentication attempts (success/failure)
- Authorization failures
- Input validation failures
- Security-relevant events

### What NOT to Log

```python
# NEVER log these
password = "secret123"
credit_card = "4111111111111111"
ssn = "123-45-6789"
api_key = "sk_live_..."
session_token = "abc123..."

# Log sanitized versions
logger.info(f"Login attempt for user: {username}")
logger.info(f"Card ending in: {credit_card[-4:]}")
logger.info(f"API key: {api_key[:8]}...")
```

### Secure Logging Pattern

```python
import logging

class SanitizingFilter(logging.Filter):
    PATTERNS = [
        (r'password["\']?\s*[:=]\s*["\']?[^"\'&\s]+', 'password=***'),
        (r'api[_-]?key["\']?\s*[:=]\s*["\']?[^"\'&\s]+', 'api_key=***'),
        (r'\b\d{16}\b', '****-****-****-****'),  # Credit cards
    ]

    def filter(self, record):
        import re
        msg = record.getMessage()
        for pattern, replacement in self.PATTERNS:
            msg = re.sub(pattern, replacement, msg, flags=re.I)
        record.msg = msg
        record.args = ()
        return True
```

---

## Checklist

- [ ] All SQL uses parameterized queries
- [ ] ORDER BY and table names whitelisted
- [ ] URL fetching validates scheme, resolves DNS, checks IP
- [ ] SSRF: DNS rebinding prevented
- [ ] SSRF: Redirects validated at each hop
- [ ] XML parsing has external entities disabled
- [ ] File paths canonicalized and validated against base
- [ ] Filenames sanitized
- [ ] Shell commands avoided or use subprocess without shell=True
- [ ] Sensitive data never logged
- [ ] Error messages don't expose internals
