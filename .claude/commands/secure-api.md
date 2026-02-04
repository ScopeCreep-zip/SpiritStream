---
name: Secure API Design
description: Use when building REST APIs, GraphQL endpoints, WebSocket connections, or implementing CORS. Covers modern API security patterns.
allowed-tools:
  - Read
  - Grep
  - Glob
  - Edit
  - Write
---

# Secure API Design Guide

Apply these practices when building APIs.

## REST API Security

### Authentication

**API Key Best Practices:**
- Transmit in header, not URL: `Authorization: ApiKey <key>`
- Hash keys in database (like passwords)
- Support key rotation (multiple active keys)
- Scope keys to specific permissions
- Log key usage for auditing

**Bearer Token Pattern:**
```
Authorization: Bearer <token>
```

### Rate Limiting

**Implementation:**
```python
from functools import wraps
import time

# Simple token bucket
class RateLimiter:
    def __init__(self, requests_per_minute):
        self.rpm = requests_per_minute
        self.requests = {}

    def is_allowed(self, key):
        now = time.time()
        minute = int(now / 60)

        if key not in self.requests or self.requests[key][0] != minute:
            self.requests[key] = [minute, 0]

        if self.requests[key][1] >= self.rpm:
            return False

        self.requests[key][1] += 1
        return True

limiter = RateLimiter(100)  # 100 req/min

def rate_limit(f):
    @wraps(f)
    def wrapper(*args, **kwargs):
        key = get_client_identifier()  # IP or user ID
        if not limiter.is_allowed(key):
            return {'error': 'Rate limit exceeded'}, 429
        return f(*args, **kwargs)
    return wrapper
```

**Rate Limit Headers:**
```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 45
X-RateLimit-Reset: 1640000000
Retry-After: 30
```

**Limits to Consider:**
| Endpoint Type | Typical Limit |
|--------------|---------------|
| Public API | 100/min |
| Authenticated | 1000/min |
| Login attempts | 5/min per account |
| Password reset | 3/hour |
| File upload | 10/hour |

### Request Validation

```python
from pydantic import BaseModel, validator
from typing import List, Optional

class CreateUserRequest(BaseModel):
    email: str
    name: str
    roles: Optional[List[str]] = []

    @validator('roles')
    def validate_roles(cls, v):
        allowed = {'user', 'editor'}  # Never allow 'admin' via API
        if not set(v).issubset(allowed):
            raise ValueError('Invalid role')
        return v

@app.post('/users')
def create_user(request: CreateUserRequest):
    # Pydantic validates automatically
    pass
```

### Response Security

```python
# Filter sensitive fields
def user_to_dict(user):
    return {
        'id': user.id,
        'email': user.email,
        'name': user.name
        # Exclude: password_hash, api_keys, internal_notes
    }

# Consistent error format (don't leak internals)
def error_response(message, status=400):
    return {
        'error': {
            'message': message,
            'code': status
        }
    }, status
```

---

## GraphQL Security

### Query Complexity Limits

```python
# Prevent deeply nested queries
from graphql import parse

def calculate_complexity(query, max_depth=10, max_complexity=1000):
    ast = parse(query)

    def traverse(node, depth=0):
        if depth > max_depth:
            raise QueryTooComplex("Max depth exceeded")

        complexity = 1
        for child in getattr(node, 'selection_set', {}).get('selections', []):
            complexity += traverse(child, depth + 1)
        return complexity

    total = traverse(ast.definitions[0])
    if total > max_complexity:
        raise QueryTooComplex("Query too complex")
```

### Disable Introspection in Production

```python
# graphene-python
schema = graphene.Schema(
    query=Query,
    mutation=Mutation,
    auto_camelcase=True
)

# In production, disable introspection
if not DEBUG:
    schema = graphene.Schema(
        query=Query,
        mutation=Mutation,
        introspection=False  # Disable __schema queries
    )
```

### Field-Level Authorization

```python
import strawberry
from strawberry.types import Info

@strawberry.type
class User:
    id: str
    email: str

    @strawberry.field
    def ssn(self, info: Info) -> str:
        # Only admins can see SSN
        if not info.context.user.is_admin:
            raise PermissionError("Unauthorized")
        return self._ssn

    @strawberry.field
    def orders(self, info: Info) -> List['Order']:
        # Users can only see their own orders
        if info.context.user.id != self.id and not info.context.user.is_admin:
            return []
        return self._orders
```

### Batching Attack Prevention

```python
# Limit batch size for mutations
@strawberry.mutation
def create_users(self, users: List[CreateUserInput]) -> List[User]:
    if len(users) > 10:
        raise ValueError("Maximum 10 users per batch")
    # ...
```

### GraphQL Security Checklist

- [ ] Query depth limited
- [ ] Query complexity calculated and limited
- [ ] Introspection disabled in production
- [ ] Field-level authorization implemented
- [ ] Batch operations limited
- [ ] Persisted queries for production (optional but recommended)
- [ ] Rate limiting per operation type

---

## WebSocket Security

### Connection Authentication

```javascript
// Server (Node.js with ws)
const WebSocket = require('ws');
const jwt = require('jsonwebtoken');

const wss = new WebSocket.Server({ noServer: true });

server.on('upgrade', (request, socket, head) => {
  // Authenticate before upgrade
  const token = new URL(request.url, 'http://localhost').searchParams.get('token');

  try {
    const user = jwt.verify(token, SECRET);
    wss.handleUpgrade(request, socket, head, (ws) => {
      ws.user = user;
      wss.emit('connection', ws, request);
    });
  } catch (e) {
    socket.write('HTTP/1.1 401 Unauthorized\r\n\r\n');
    socket.destroy();
  }
});
```

### Message Validation

```javascript
wss.on('connection', (ws) => {
  ws.on('message', (data) => {
    let message;
    try {
      message = JSON.parse(data);
    } catch {
      ws.close(1008, 'Invalid JSON');
      return;
    }

    // Validate message structure
    if (!message.type || typeof message.type !== 'string') {
      ws.close(1008, 'Invalid message format');
      return;
    }

    // Validate message type
    const allowedTypes = ['subscribe', 'unsubscribe', 'ping'];
    if (!allowedTypes.includes(message.type)) {
      ws.close(1008, 'Unknown message type');
      return;
    }

    // Handle message...
  });
});
```

### WebSocket Rate Limiting

```javascript
const messageCount = new Map();

ws.on('message', () => {
  const count = (messageCount.get(ws) || 0) + 1;
  messageCount.set(ws, count);

  if (count > 100) {  // 100 messages per interval
    ws.close(1008, 'Rate limit exceeded');
    return;
  }
});

// Reset counts periodically
setInterval(() => messageCount.clear(), 60000);
```

### Authorization per Channel/Topic

```javascript
ws.on('message', (data) => {
  const { type, channel } = JSON.parse(data);

  if (type === 'subscribe') {
    // Verify user can access this channel
    if (!canUserAccessChannel(ws.user, channel)) {
      ws.send(JSON.stringify({ error: 'Unauthorized' }));
      return;
    }
    subscribeToChannel(ws, channel);
  }
});
```

---

## CORS Configuration

### Secure Configuration

```python
# Flask-CORS
from flask_cors import CORS

CORS(app,
  origins=['https://app.yourdomain.com', 'https://admin.yourdomain.com'],
  methods=['GET', 'POST', 'PUT', 'DELETE'],
  allow_headers=['Content-Type', 'Authorization', 'X-CSRF-Token'],
  supports_credentials=True,  # Only if needed for cookies
  max_age=3600
)
```

**CORS Headers:**
```
Access-Control-Allow-Origin: https://app.yourdomain.com
Access-Control-Allow-Methods: GET, POST, PUT, DELETE
Access-Control-Allow-Headers: Content-Type, Authorization
Access-Control-Allow-Credentials: true
Access-Control-Max-Age: 3600
```

### CORS Security Rules

| Rule | Why |
|------|-----|
| Never use `*` with credentials | Browsers block this anyway |
| Whitelist specific origins | Don't reflect Origin header |
| Validate Origin header | Check against allowlist server-side |
| Limit allowed methods | Only what's needed |
| Limit allowed headers | Only what's needed |

### CORS Checklist

- [ ] Specific origins whitelisted (no wildcards in production)
- [ ] Origin validated server-side
- [ ] Credentials only enabled if needed
- [ ] Methods restricted to required ones
- [ ] Headers restricted to required ones

---

## API Versioning Security

```
/api/v1/users  → Keep supported
/api/v2/users  → Current version
```

- Deprecate old versions with timeline
- Don't maintain security patches for EOL versions
- Log usage of deprecated versions
- Return deprecation headers:
```
Deprecation: true
Sunset: Sat, 1 Jan 2025 00:00:00 GMT
```

---

## Checklist

- [ ] API authentication required (API key or Bearer token)
- [ ] Rate limiting implemented
- [ ] Request bodies validated with schema
- [ ] Responses filtered to exclude sensitive data
- [ ] GraphQL complexity limits set
- [ ] GraphQL introspection disabled in production
- [ ] WebSocket connections authenticated
- [ ] WebSocket messages validated
- [ ] CORS origins explicitly whitelisted
- [ ] Error messages don't leak internals
